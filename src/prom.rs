/*
 * Copyright 2026 Federico D'Ambrosio
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
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// Range query results kept for re-use, such as when panning back to a window.
const CACHE_CAPACITY: usize = 64;
/// Largest response body read from Prometheus.
const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
/// Longest excerpt of a response body included in an error message.
const ERROR_EXCERPT_CHARS: usize = 512;

type QueryWaiter = tokio::sync::oneshot::Sender<Result<Vec<Series>, String>>;
type InflightQueries = Arc<Mutex<HashMap<CacheKey, Vec<QueryWaiter>>>>;

/// A range query's identity: expression, start, end, and step.
type CacheKey = (String, i64, i64, Duration);

/// Recent range query results, evicting the oldest beyond `CACHE_CAPACITY`.
#[derive(Debug, Default)]
struct QueryCache {
    entries: HashMap<CacheKey, Vec<Series>>,
    order: VecDeque<CacheKey>,
}

impl QueryCache {
    fn get(&self, key: &CacheKey) -> Option<Vec<Series>> {
        self.entries.get(key).cloned()
    }

    fn insert(&mut self, key: CacheKey, series: Vec<Series>) {
        if self.entries.insert(key.clone(), series).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > CACHE_CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

/// Locks `mutex`, recovering the data if a panic poisoned it. The guarded maps
/// stay consistent across a panic because every update is a single insert or
/// remove.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The first caller's claim on an in-flight range query, which identical
/// concurrent queries wait on.
///
/// Dropping the guard removes the claim, so a request whose future is dropped
/// mid-flight cannot leave later identical requests waiting forever: their
/// senders are dropped and they fail with "inflight request cancelled".
struct InflightGuard {
    inflight: InflightQueries,
    key: CacheKey,
}

impl InflightGuard {
    fn take_waiters(&self) -> Vec<QueryWaiter> {
        lock(&self.inflight).remove(&self.key).unwrap_or_default()
    }

    /// Sends the leader's result to every waiting caller and releases the claim.
    fn publish(self, result: &Result<Vec<Series>>) {
        for waiter in self.take_waiters() {
            let _ = waiter.send(match result {
                Ok(series) => Ok(series.clone()),
                Err(error) => Err(error.to_string()),
            });
        }
    }
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        drop(self.take_waiters());
    }
}

/// A response body over the size limit. It is not retried, since the same
/// query would return the same oversized result.
#[derive(Debug)]
struct ResponseTooLarge(usize);

impl std::fmt::Display for ResponseTooLarge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "response exceeds the {} byte limit", self.0)
    }
}

impl std::error::Error for ResponseTooLarge {}

/// Prometheus could not be reached, or the connection failed mid-response, as
/// opposed to Prometheus answering with an error.
#[derive(Debug)]
struct TransportError(String);

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TransportError {}

/// Prometheus answered with an error status.
#[derive(Debug)]
struct StatusError {
    status: reqwest::StatusCode,
    message: String,
}

impl std::fmt::Display for StatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "prometheus {}: {}", self.status, self.message)
    }
}

impl std::error::Error for StatusError {}

impl StatusError {
    /// Uses the `errorType` and `error` of a Prometheus error body, falling
    /// back to an excerpt of the body for proxies and other servers.
    fn new(status: reqwest::StatusCode, body: &str) -> Self {
        #[derive(Deserialize)]
        struct ErrorBody {
            #[serde(rename = "errorType")]
            error_type: Option<String>,
            error: Option<String>,
        }
        let message = match serde_json::from_str::<ErrorBody>(body) {
            Ok(ErrorBody {
                error_type,
                error: Some(error),
            }) => match error_type {
                Some(error_type) => format!("{error_type}: {error}"),
                None => error,
            },
            _ => excerpt(body),
        };
        Self { status, message }
    }
}

/// Whether trying again could succeed: the connection failed, or Prometheus
/// was overloaded or unavailable. A rejected query fails the same way again.
fn is_retryable(error: &anyhow::Error) -> bool {
    if error.is::<TransportError>() {
        return true;
    }
    error.downcast_ref::<StatusError>().is_some_and(|error| {
        error.status.is_server_error() || error.status == reqwest::StatusCode::TOO_MANY_REQUESTS
    })
}

/// The start of a response body, for error messages.
fn excerpt(text: &str) -> String {
    match text.char_indices().nth(ERROR_EXCERPT_CHARS) {
        Some((end, _)) => format!("{}… ({} bytes)", &text[..end], text.len()),
        None => text.to_string(),
    }
}

/// A simple Prometheus HTTP client.
#[derive(Debug, Clone)]
pub(crate) struct PromClient {
    /// Base URL of the Prometheus server.
    pub(crate) base: String,
    /// HTTP client.
    client: reqwest::Client,
    /// Recent range query results.
    cache: Arc<Mutex<QueryCache>>,
    /// In-flight requests: key -> list of waiters
    inflight: InflightQueries,
    /// Largest response body accepted.
    max_response_bytes: usize,
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
            cache: Arc::new(Mutex::new(QueryCache::default())),
            inflight: Arc::new(Mutex::new(HashMap::new())),
            max_response_bytes: MAX_RESPONSE_BYTES,
        }
    }

    pub(crate) fn build_query_range_url(
        &self,
        expr: &str,
        start: i64,
        end: i64,
        step: Duration,
    ) -> String {
        // Whole seconds keep URLs readable; Prometheus also accepts `ms` steps.
        let step_param = if step.subsec_millis() == 0 {
            format!("{}s", step.as_secs().max(1))
        } else {
            format!("{}ms", step.as_millis())
        };
        format!(
            "{}/api/v1/query_range?query={}&start={}&end={}&step={}",
            self.base.trim_end_matches('/'),
            urlencoding::encode(expr),
            start,
            end,
            step_param
        )
    }

    pub(crate) fn build_query_url(&self, expr: &str, time: i64) -> String {
        format!(
            "{}/api/v1/query?query={}&time={}",
            self.base.trim_end_matches('/'),
            urlencoding::encode(expr),
            time
        )
    }

    pub(crate) async fn query_range(
        &self,
        expr: &str,
        start: i64,
        end: i64,
        step: Duration,
    ) -> Result<Vec<Series>> {
        let cache_key = (expr.to_string(), start, end, step);
        if let Some(series) = lock(&self.cache).get(&cache_key) {
            return Ok(series);
        }

        let inflight_key = cache_key.clone();
        let claim = {
            let mut inflight = lock(&self.inflight);
            if let Some(waiters) = inflight.get_mut(&inflight_key) {
                let (tx, rx) = tokio::sync::oneshot::channel();
                waiters.push(tx);
                Err(rx)
            } else {
                inflight.insert(inflight_key.clone(), Vec::new());
                Ok(InflightGuard {
                    inflight: Arc::clone(&self.inflight),
                    key: inflight_key,
                })
            }
        };
        let claim = match claim {
            Ok(claim) => claim,
            Err(rx) => {
                return match rx.await {
                    Ok(Ok(res)) => Ok(res),
                    Ok(Err(s)) => Err(anyhow!(s)),
                    Err(_) => Err(anyhow!("inflight request cancelled")),
                };
            }
        };

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
                    lock(&self.cache).insert(cache_key, series.clone());
                    final_res = Ok(series);
                    break;
                }
                Err(e) if !is_retryable(&e) => {
                    last_err = e;
                    break;
                }
                Err(e) => last_err = e,
            }
        }

        if final_res.is_err() {
            final_res = Err(last_err);
        }

        claim.publish(&final_res);
        final_res
    }

    async fn perform_request(&self, url: &str) -> Result<Vec<Series>> {
        let text = self.get_text(url).await?;

        let body: PromResponse<QueryRangeData> = serde_json::from_str(&text)
            .map_err(|e| anyhow!("parsing json: {} (body: {})", e, excerpt(&text)))?;

        if body.status != "success" {
            return Err(anyhow!(
                "prometheus error status: {} — body: {}",
                body.status,
                excerpt(&text)
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
        let url = self.build_query_url(expr, time);
        let body: PromResponse<QueryInstantData> = self.get_json(&url).await?;
        ensure_success(&body.status)?;
        Ok(body.data.result_strings())
    }

    pub(crate) async fn query_instant_series(&self, expr: &str, time: i64) -> Result<Vec<Series>> {
        let url = self.build_query_url(expr, time);
        let body: PromResponse<QueryInstantData> = self.get_json(&url).await?;
        ensure_success(&body.status)?;
        Ok(body.data.into_series(time))
    }

    async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let text = self.get_text(url).await?;
        serde_json::from_str(&text)
            .map_err(|e| anyhow!("parsing json: {} (body: {})", e, excerpt(&text)))
    }

    /// Reads a response body of at most `max_response_bytes`, so one huge
    /// result cannot exhaust memory.
    async fn get_text(&self, url: &str) -> Result<String> {
        let mut resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| TransportError(format!("request failed: {e}")))?;
        let status = resp.status();
        let limit = self.max_response_bytes;
        let too_large = || anyhow::Error::new(ResponseTooLarge(limit));
        if resp
            .content_length()
            .is_some_and(|length| length > limit as u64)
        {
            return Err(too_large());
        }
        let mut body = Vec::new();
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| TransportError(format!("reading text: {e}")))?
        {
            if body.len() + chunk.len() > limit {
                return Err(too_large());
            }
            body.extend_from_slice(&chunk);
        }
        let text = String::from_utf8(body)
            .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned());

        if !status.is_success() {
            return Err(StatusError::new(status, &text).into());
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

    fn into_series(self, time: i64) -> Vec<Series> {
        match self.result_type.as_str() {
            "vector" => vector_result_series(&self.result, time),
            "scalar" => scalar_result_series(&self.result, time)
                .into_iter()
                .collect(),
            _ => Vec::new(),
        }
    }
}

fn vector_result_series(result: &serde_json::Value, time: i64) -> Vec<Series> {
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
                .and_then(|value| value.as_str())?;

            Some(Series {
                metric: metric
                    .iter()
                    .filter_map(|(label, value)| {
                        value
                            .as_str()
                            .map(|value| (label.clone(), value.to_string()))
                    })
                    .collect(),
                values: vec![(time as f64, value.to_string())],
            })
        })
        .collect()
}

fn scalar_result_series(result: &serde_json::Value, time: i64) -> Option<Series> {
    let value = result
        .as_array()
        .and_then(|value| value.get(1))
        .and_then(|value| value.as_str())?;

    Some(Series {
        metric: HashMap::new(),
        values: vec![(time as f64, value.to_string())],
    })
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const EMPTY_MATRIX: &str = r#"{"status":"success","data":{"resultType":"matrix","result":[]}}"#;

    /// A Prometheus stand-in. The first `stalled` connections are accepted and
    /// never answered; later ones receive `response` (a full HTTP response).
    async fn server(stalled: usize, response: String) -> (String, Arc<tokio::sync::Notify>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let reached = Arc::new(tokio::sync::Notify::new());
        let notify = Arc::clone(&reached);
        tokio::spawn(async move {
            let mut held = Vec::new();
            let mut accepted = 0;
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                accepted += 1;
                let mut buffer = [0_u8; 4096];
                let _ = socket.read(&mut buffer).await;
                notify.notify_one();
                if accepted <= stalled {
                    held.push(socket);
                } else {
                    let _ = socket.write_all(response.as_bytes()).await;
                }
            }
        });
        (format!("http://{address}"), reached)
    }

    fn ok_response(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    /// Answers every request with `response`, counting the requests.
    async fn counting_server(response: String) -> (String, Arc<std::sync::atomic::AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = Arc::clone(&requests);
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buffer = [0_u8; 4096];
                let _ = socket.read(&mut buffer).await;
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });
        (format!("http://{address}"), requests)
    }

    fn status_response(status: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[tokio::test]
    async fn a_dropped_query_does_not_strand_identical_queries() {
        let (url, _) = server(1, ok_response(EMPTY_MATRIX)).await;
        let client = PromClient::new(url);
        let step = Duration::from_secs(15);

        let abandoned = tokio::time::timeout(
            Duration::from_millis(200),
            client.query_range("up", 0, 60, step),
        )
        .await;
        assert!(abandoned.is_err(), "the stalled request should time out");

        let retried = tokio::time::timeout(
            Duration::from_secs(2),
            client.query_range("up", 0, 60, step),
        )
        .await
        .expect("a later identical query must not wait on the dropped one");
        assert!(retried.unwrap().is_empty());
        assert!(lock(&client.inflight).is_empty());
    }

    #[tokio::test]
    async fn waiters_fail_promptly_when_the_leading_query_is_cancelled() {
        let (url, reached) = server(1, ok_response(EMPTY_MATRIX)).await;
        let client = PromClient::new(url);
        let step = Duration::from_secs(15);
        let leader = tokio::spawn({
            let client = client.clone();
            async move { client.query_range("up", 0, 60, step).await }
        });
        reached.notified().await;
        let follower = tokio::spawn({
            let client = client.clone();
            async move { client.query_range("up", 0, 60, step).await }
        });
        while lock(&client.inflight)
            .values()
            .all(|waiters| waiters.is_empty())
        {
            tokio::task::yield_now().await;
        }

        leader.abort();

        let error = tokio::time::timeout(Duration::from_secs(2), follower)
            .await
            .expect("the follower must not hang")
            .unwrap()
            .unwrap_err();
        assert!(error.to_string().contains("cancelled"), "{error}");
        assert!(lock(&client.inflight).is_empty());
    }

    #[tokio::test]
    async fn rejected_queries_are_not_retried() {
        let body = r#"{"status":"error","errorType":"bad_data","error":"parse error at char 4"}"#;
        let (url, requests) = counting_server(status_response("400 Bad Request", body)).await;

        let error = PromClient::new(url)
            .query_range("up{", 0, 60, Duration::from_secs(15))
            .await
            .unwrap_err();

        assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(
            error.to_string(),
            "prometheus 400 Bad Request: bad_data: parse error at char 4"
        );
    }

    #[tokio::test]
    async fn too_many_requests_are_retried() {
        let body = r#"{"status":"error","errorType":"unavailable","error":"slow down"}"#;
        let (url, requests) = counting_server(status_response("429 Too Many Requests", body)).await;

        PromClient::new(url)
            .query_range("up", 0, 60, Duration::from_secs(15))
            .await
            .unwrap_err();

        assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn unavailable_prometheus_is_retried() {
        let body = r#"{"status":"error","errorType":"unavailable","error":"too many queries"}"#;
        let (url, requests) =
            counting_server(status_response("503 Service Unavailable", body)).await;

        PromClient::new(url)
            .query_range("up", 0, 60, Duration::from_secs(15))
            .await
            .unwrap_err();

        assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn malformed_success_bodies_are_not_retried() {
        let (url, requests) = counting_server(ok_response("not json")).await;

        PromClient::new(url)
            .query_range("up", 0, 60, Duration::from_secs(15))
            .await
            .unwrap_err();

        assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn non_prometheus_error_bodies_keep_an_excerpt() {
        let (url, requests) = counting_server(status_response(
            "502 Bad Gateway",
            "<html>bad gateway</html>",
        ))
        .await;

        let error = PromClient::new(url)
            .query_range("up", 0, 60, Duration::from_secs(15))
            .await
            .unwrap_err();

        assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 4);
        assert_eq!(
            error.to_string(),
            "prometheus 502 Bad Gateway: <html>bad gateway</html>"
        );
    }

    #[test]
    fn the_query_cache_keeps_only_recent_windows() {
        let mut cache = QueryCache::default();
        let step = Duration::from_secs(15);
        for end in 0..1000 {
            cache.insert(("up".to_string(), end - 60, end, step), Vec::new());
        }

        assert_eq!(cache.len(), CACHE_CAPACITY);
        assert!(cache.get(&("up".to_string(), 939, 999, step)).is_some());
        assert!(cache.get(&("up".to_string(), -60, 0, step)).is_none());
    }

    #[tokio::test]
    async fn oversized_responses_are_rejected() {
        let body = format!("{{\"padding\":\"{}\"}}", "x".repeat(4096));
        // With a declared length, and streamed until the connection closes.
        let streamed = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}"
        );
        for response in [ok_response(&body), streamed] {
            let (url, requests) = counting_server(response).await;
            let mut client = PromClient::new(url);
            client.max_response_bytes = 1024;

            let error = client
                .query_range("up", 0, 60, Duration::from_secs(15))
                .await
                .unwrap_err();

            assert!(error.to_string().contains("byte limit"), "{error}");
            assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn error_excerpts_are_truncated_on_character_boundaries() {
        assert_eq!(excerpt("short"), "short");
        let long = "é".repeat(ERROR_EXCERPT_CHARS + 10);
        let shown = excerpt(&long);
        assert!(shown.starts_with(&"é".repeat(ERROR_EXCERPT_CHARS)));
        assert!(
            shown.ends_with(&format!("… ({} bytes)", long.len())),
            "{shown}"
        );
    }

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
    fn query_range_url_keeps_sub_second_steps() {
        let client = PromClient::new("http://localhost:9090".to_string());
        let url = client.build_query_range_url("up", 0, 60, Duration::from_millis(1500));
        assert!(url.ends_with("&step=1500ms"), "{url}");
    }

    #[test]
    fn range_query_identity_keeps_the_full_step_duration() {
        let one_second: CacheKey = ("up".into(), 0, 60, Duration::from_secs(1));
        let fifteen_hundred_ms: CacheKey = ("up".into(), 0, 60, Duration::from_millis(1500));

        assert_ne!(one_second, fifteen_hundred_ms);
    }

    #[test]
    fn test_build_query_url_preserves_path_prefix() {
        let client = PromClient::new("http://localhost:9090/prometheus/".to_string());
        let url = client.build_query_url("up{job=\"node\"}", 1600003600);

        assert_eq!(
            url,
            "http://localhost:9090/prometheus/api/v1/query?query=up%7Bjob%3D%22node%22%7D&time=1600003600"
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

    #[test]
    fn test_query_instant_vector_converts_to_series() {
        let json = r#"
        {
            "resultType": "vector",
            "result": [
                {
                    "metric": { "instance": "node-1", "job": "node" },
                    "value": [1435781451.781, "1"]
                },
                {
                    "metric": { "instance": "node-2", "job": "node" },
                    "value": [1435781451.781, "2.5"]
                }
            ]
        }
        "#;

        let data: QueryInstantData = serde_json::from_str(json).unwrap();
        let series = data.into_series(1_435_781_451);

        assert_eq!(series.len(), 2);
        assert_eq!(series[0].metric.get("instance").unwrap(), "node-1");
        assert_eq!(series[0].values, vec![(1_435_781_451.0, "1".to_string())]);
        assert_eq!(series[1].metric.get("instance").unwrap(), "node-2");
        assert_eq!(series[1].values, vec![(1_435_781_451.0, "2.5".to_string())]);
    }

    #[test]
    fn test_query_instant_scalar_converts_to_unlabeled_series() {
        let json = r#"
        {
            "resultType": "scalar",
            "result": [1435781451.781, "42"]
        }
        "#;

        let data: QueryInstantData = serde_json::from_str(json).unwrap();
        let series = data.into_series(1_435_781_451);

        assert_eq!(series.len(), 1);
        assert!(series[0].metric.is_empty());
        assert_eq!(series[0].values, vec![(1_435_781_451.0, "42".to_string())]);
    }
}
