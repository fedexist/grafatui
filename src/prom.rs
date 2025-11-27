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
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A simple Prometheus HTTP client.
#[derive(Debug, Clone)]
pub struct PromClient {
    /// Base URL of the Prometheus server.
    pub base: String,
    /// HTTP client.
    client: reqwest::Client,
    /// Query cache: expr -> (start, end, step, data)
    cache: Arc<Mutex<HashMap<String, (i64, i64, Duration, Vec<Series>)>>>,
    /// In-flight requests: key -> list of waiters
    inflight:
        Arc<Mutex<HashMap<String, Vec<tokio::sync::oneshot::Sender<Result<Vec<Series>, String>>>>>>,
}

impl PromClient {
    pub fn new(base: String) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            base,
            client: http,
            cache: Arc::new(Mutex::new(HashMap::new())),
            inflight: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn build_query_range_url(
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

    pub async fn query_range(
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

        // Check in-flight
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
                    // Update cache
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

        let body: QueryRangeResponse = serde_json::from_str(&text)
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
}

#[derive(Debug, Deserialize, Clone)]
pub struct QueryRangeResponse {
    pub status: String,
    pub data: QueryRangeData,
}

#[derive(Debug, Deserialize, Clone)]
pub struct QueryRangeData {
    #[serde(rename = "resultType")]
    #[allow(dead_code)]
    pub result_type: String,
    pub result: Vec<Series>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Series {
    pub metric: std::collections::HashMap<String, String>,
    pub values: Vec<(f64, String)>, // (ts, value)
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

        let resp: QueryRangeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, "success");
        assert_eq!(resp.data.result_type, "matrix");
        assert_eq!(resp.data.result.len(), 1);
        assert_eq!(resp.data.result[0].metric.get("job").unwrap(), "prometheus");
        assert_eq!(resp.data.result[0].values.len(), 2);
    }
}
