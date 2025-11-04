use anyhow::{Result, anyhow};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PromClient {
    pub base: String,
    http: Client,
}

impl PromClient {
    pub fn new(base: String) -> Self {
        Self {
            base,
            http: Client::new(),
        }
    }

    pub async fn query_range(
        &self,
        expr: &str,
        range: Duration,
        step: Duration,
    ) -> Result<Vec<Series>> {
        let end = Utc::now().timestamp();
        let start = end - (range.as_secs() as i64);
        let step_s = step.as_secs().max(1);
        let step_param = format!("{}s", step_s);
        let url = format!(
            "{}/api/v1/query_range?query={}&start={}&end={}&step={}",
            self.base.trim_end_matches('/'),
            urlencoding::encode(expr),
            start,
            end,
            step_param
        );

        // Better error visibility: read text to surface server messages (e.g., invalid step)
        let resp = self.http.get(&url).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow!("prometheus {}: {}", status, text));
        }
        let body: QueryRangeResponse = serde_json::from_str(&text)?;
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
    pub resultType: String,
    pub result: Vec<Series>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Series {
    pub metric: std::collections::HashMap<String, String>,
    pub values: Vec<(f64, String)>, // (ts, value)
}
