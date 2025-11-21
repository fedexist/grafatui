use anyhow::{Result, anyhow};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

/// A simple Prometheus HTTP client.
#[derive(Debug, Clone)]
pub struct PromClient {
    /// Base URL of the Prometheus server.
    pub base: String,
    /// HTTP client.
    client: reqwest::Client,
}

impl PromClient {
    pub fn new(base: String) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { base, client: http }
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
        range: Duration,
        step: Duration,
    ) -> Result<Vec<Series>> {
        let end = Utc::now().timestamp();
        let start = end - (range.as_secs() as i64);
        let url = self.build_query_range_url(expr, start, end, step);

        let max_retries = 3;
        let mut last_err = anyhow!("unknown error");

        for attempt in 0..=max_retries {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(100 * (1 << attempt))).await;
            }

            match self.perform_request(&url).await {
                Ok(series) => return Ok(series),
                Err(e) => last_err = e,
            }
        }

        Err(last_err)
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
