/*
 * Copyright 2025 Federico D'Ambrosio
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

type QueryCache = Arc<Mutex<HashMap<String, (i64, i64, Duration, Vec<Series>)>>>;
type QueryWaiter = tokio::sync::oneshot::Sender<Result<Vec<Series>, String>>;
type InflightQueries = Arc<Mutex<HashMap<String, Vec<QueryWaiter>>>>;

/// A simple Prometheus HTTP client.
#[derive(Debug, Clone)]
pub(crate) struct PromClient {
    /// Base URL of the Prometheus server.
    pub(crate) base: String,
    /// HTTP client.
    client: reqwest::Client,
    /// Query cache: expr -> (start, end, step, data)
    cache: QueryCache,
    /// In-flight requests: key -> list of waiters
    inflight: InflightQueries,
}

impl PromClient {
    pub(crate) fn new(base: String) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|e| {
                eprintln!(
                    "Warning: Failed to configure HTTP client with timeouts: {}",
                    e
                );
                eprintln!("         Falling back to default client (requests may hang).");
                Client::new()
            });

        Self {
            base,
            client: http,
            cache: Arc::new(Mutex::new(HashMap::new())),
            inflight: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn build_query_range_url(
        &self,
        expr: &str,
        start: i64,
        end: i64,
        step: Duration,
    ) -> String {
        let step_s = step.as_secs().max(1);
        let step_param = format!("{}s", step_s);
        format!(
            "{}/api/v1/query_range?query={}&start={}&end={}&step={}",
            self.base.trim_end_matches('/'),
            urlencoding::encode(expr),
            start,
            end,
            step_param
        )
    }

    pub(crate) async fn query_range(
        &self,
        expr: &str,
        start: i64,
        end: i64,
        step: Duration,
    ) -> Result<Vec<Series>> {
        // Check cache
        {
            let cache = self.cache.lock().unwrap();
            if let Some((c_start, c_end, c_step, data)) = cache.get(expr) {
                if *c_start == start && *c_end == end && *c_step == step {
                    return Ok(data.clone());
                }
            }
        }

        let inflight_key = format!("{}|{}|{}|{}", expr, start, end, step.as_secs());
        let rx = {
            let mut inflight = self.inflight.lock().unwrap();
            if let Some(waiters) = inflight.get_mut(&inflight_key) {
                let (tx, rx) = tokio::sync::oneshot::channel();
                waiters.push(tx);
                Some(rx)
            } else {
                inflight.insert(inflight_key.clone(), Vec::new());
                None
            }
        };

        if let Some(rx) = rx {
            return match rx.await {
                Ok(Ok(res)) => Ok(res),
                Ok(Err(s)) => Err(anyhow!(s)),
                Err(_) => Err(anyhow!("inflight request cancelled")),
            };
        }

        let url = self.build_query_range_url(expr, start, end, step);

        let max_retries = 3;
        let mut last_err = anyhow!("unknown error");
        let mut final_res = Err(anyhow!("unknown error"));

        for attempt in 0..=max_retries {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(100 * (1 << attempt))).await;
            }

            match self.perform_request(&url).await {
                Ok(series) => {
                    {
                        let mut cache = self.cache.lock().unwrap();
                        cache.insert(expr.to_string(), (start, end, step, series.clone()));
                    }
                    final_res = Ok(series);
                    break;
                }
                Err(e) => last_err = e,
            }
        }

        if final_res.is_err() {
            final_res = Err(last_err);
        }

        // Notify waiters
        {
            let mut inflight = self.inflight.lock().unwrap();
            if let Some(waiters) = inflight.remove(&inflight_key) {
                for tx in waiters {
                    let _ = tx.send(match &final_res {
                        Ok(v) => Ok(v.clone()),
                        Err(e) => Err(e.to_string()),
                    });
                }
            }
        }

        final_res
    }

    async fn perform_request(&self, url: &str) -> Result<Vec<Series>> {
        let text = self.get_text(url).await?;

        let body: PromResponse<QueryRangeData> = serde_json::from_str(&text)
            .map_err(|e| anyhow!("parsing json: {} (body: {})", e, text))?;

        if body.status != "success" {
            return Err(anyhow!(
                "prometheus error status: {} — body: {}",
                body.status,
                text
            ));
        }

        Ok(body.data.result)
    }

    pub(crate) async fn label_values(&self, label: &str) -> Result<Vec<String>> {
        let url = format!(
            "{}/api/v1/label/{}/values",
            self.base.trim_end_matches('/'),
            urlencoding::encode(label)
        );
        let body: PromResponse<Vec<String>> = self.get_json(&url).await?;
        ensure_success(&body.status)?;
        Ok(body.data)
    }

    pub(crate) async fn series_label_values(
        &self,
        metric: &str,
        label: &str,
        start: i64,
        end: i64,
    ) -> Result<Vec<String>> {
        let url = format!(
            "{}/api/v1/series?match[]={}&start={}&end={}",
            self.base.trim_end_matches('/'),
            urlencoding::encode(metric),
            start,
            end
        );
        let body: PromResponse<Vec<HashMap<String, String>>> = self.get_json(&url).await?;
        ensure_success(&body.status)?;
        Ok(body
            .data
            .into_iter()
            .filter_map(|series| series.get(label).cloned())
            .collect())
    }

    pub(crate) async fn query_instant_result_strings(
        &self,
        expr: &str,
        time: i64,
    ) -> Result<Vec<String>> {
        let url = format!(
            "{}/api/v1/query?query={}&time={}",
            self.base.trim_end_matches('/'),
            urlencoding::encode(expr),
            time
        );
        let body: PromResponse<QueryInstantData> = self.get_json(&url).await?;
        ensure_success(&body.status)?;
        Ok(body.data.result_strings())
    }

    async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let text = self.get_text(url).await?;
        serde_json::from_str(&text).map_err(|e| anyhow!("parsing json: {} (body: {})", e, text))
    }

    async fn get_text(&self, url: &str) -> Result<String> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| anyhow!("request failed: {}", e))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| anyhow!("reading text: {}", e))?;

        if !status.is_success() {
            return Err(anyhow!("prometheus {}: {}", status, text));
        }

        Ok(text)
    }
}

fn ensure_success(status: &str) -> Result<()> {
    if status == "success" {
        Ok(())
    } else {
        Err(anyhow!("prometheus error status: {}", status))
    }
}

#[derive(Debug, Deserialize, Clone)]
struct PromResponse<T> {
    status: String,
    data: T,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct QueryRangeData {
    #[serde(rename = "resultType")]
    #[allow(dead_code)]
    pub(crate) result_type: String,
    pub(crate) result: Vec<Series>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Series {
    pub(crate) metric: std::collections::HashMap<String, String>,
    pub(crate) values: Vec<(f64, String)>, // (ts, value)
}

#[derive(Debug, Deserialize, Clone)]
struct QueryInstantData {
    #[serde(rename = "resultType")]
    result_type: String,
    result: serde_json::Value,
}

impl QueryInstantData {
    fn result_strings(self) -> Vec<String> {
        match self.result_type.as_str() {
            "vector" => vector_result_strings(&self.result),
            "scalar" | "string" => scalar_result_string(&self.result).into_iter().collect(),
            _ => Vec::new(),
        }
    }
}

fn vector_result_strings(result: &serde_json::Value) -> Vec<String> {
    result
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|sample| {
            let metric = sample.get("metric")?.as_object()?;
            let value = sample
                .get("value")
                .and_then(|value| value.as_array())
                .and_then(|value| value.get(1))
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let mut labels: Vec<_> = metric
                .iter()
                .filter_map(|(label, value)| value.as_str().map(|value| (label, value)))
                .collect();
            labels.sort_by(|a, b| a.0.cmp(b.0));
            let labels = labels
                .into_iter()
                .map(|(label, value)| format!("{}=\"{}\"", label, value))
                .collect::<Vec<_>>()
                .join(", ");

            if labels.is_empty() {
                Some(value.to_string())
            } else {
                Some(format!("{{{}}} {}", labels, value))
            }
        })
        .collect()
}

fn scalar_result_string(result: &serde_json::Value) -> Option<String> {
    result
        .as_array()
        .and_then(|value| value.get(1))
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_query_range_url() {
        let client = PromClient::new("http://localhost:9090".to_string());
        let expr = "up{job=\"node\"}";
        let start = 1600000000;
        let end = 1600003600;
        let step = Duration::from_secs(60);

        let url = client.build_query_range_url(expr, start, end, step);
        assert_eq!(
            url,
            "http://localhost:9090/api/v1/query_range?query=up%7Bjob%3D%22node%22%7D&start=1600000000&end=1600003600&step=60s"
        );
    }

    #[test]
    fn test_deserialize_query_range_response() {
        let json = r#"
        {
            "status": "success",
            "data": {
                "resultType": "matrix",
                "result": [
                    {
                        "metric": {
                            "__name__": "up",
                            "job": "prometheus"
                        },
                        "values": [
                            [1435781451.781, "1"],
                            [1435781466.781, "1"]
                        ]
                    }
                ]
            }
        }
        "#;

        let resp: PromResponse<QueryRangeData> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, "success");
        assert_eq!(resp.data.result_type, "matrix");
        assert_eq!(resp.data.result.len(), 1);
        assert_eq!(resp.data.result[0].metric.get("job").unwrap(), "prometheus");
        assert_eq!(resp.data.result[0].values.len(), 2);
    }

    #[test]
    fn test_query_instant_vector_result_strings() {
        let json = r#"
        {
            "resultType": "vector",
            "result": [
                {
                    "metric": { "instance": "node-1", "job": "node" },
                    "value": [1435781451.781, "1"]
                }
            ]
        }
        "#;

        let data: QueryInstantData = serde_json::from_str(json).unwrap();

        assert_eq!(
            data.result_strings(),
            vec![r#"{instance="node-1", job="node"} 1"#]
        );
    }
}
