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

use super::data::{QueryIntervals, expand_expr};
use crate::grafana::TemplateQueryVar;
use crate::prom;
use anyhow::{Result, anyhow};
use regex::Regex;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// Replaces `$name` and `${name}` references whose name `lookup` resolves.
pub(crate) fn substitute_variables<'a>(
    text: &str,
    mut lookup: impl FnMut(&str) -> Option<Cow<'a, str>>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(position) = rest.find('$') {
        out.push_str(&rest[..position]);
        let after = &rest[position + 1..];
        let (name, consumed) = if let Some(braced) = after.strip_prefix('{') {
            match braced.find('}') {
                Some(end) => (&braced[..end], end + 2),
                None => ("", 0),
            }
        } else {
            let end = after
                .find(|ch: char| !is_variable_name_char(ch))
                .unwrap_or(after.len());
            (&after[..end], end)
        };
        let value = (!name.is_empty() && name.chars().all(is_variable_name_char))
            .then(|| lookup(name))
            .flatten();
        match value {
            Some(value) => {
                out.push_str(&value);
                rest = &after[consumed..];
            }
            None => {
                out.push('$');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn is_variable_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

pub(crate) async fn refresh_scoped_variables(
    prometheus: &prom::PromClient,
    state: &mut crate::dashboard::variables::VariableState,
    active_scopes: &HashSet<usize>,
    context: VariableRefreshContext,
    vars: &HashMap<String, String>,
) {
    let VariableRefreshContext {
        update,
        range,
        intervals,
        end_ts,
    } = context;
    let mut changed_by_scope = vec![HashSet::<String>::new(); state.scopes.len()];
    for scope in 0..state.scopes.len() {
        if !active_scopes.contains(&scope) {
            continue;
        }
        let mut changed = state.scopes[scope]
            .parent
            .and_then(|parent| changed_by_scope.get(parent))
            .cloned()
            .unwrap_or_default();
        for index in 0..state.scopes[scope].variables.len() {
            let variable = &state.scopes[scope].variables[index];
            if scope == 0 && state.overrides.contains_key(&variable.name) {
                continue;
            }
            let Some(query) = variable.query.clone() else {
                continue;
            };
            let changed_names = changed.iter().map(String::as_str).collect::<Vec<_>>();
            let due = !variable.resolved
                || (update == VariableUpdate::TimeRangeChange
                    && (query.refresh == crate::grafana::VariableRefresh::OnTimeRangeChange
                        || variable.last_error.is_some()))
                || references_any(&query.query, &changed_names);
            if !due {
                continue;
            }
            let previous = variable.query_value();
            let values = state.scope_values(scope, vars);
            let result = resolve_query_variable_values(
                prometheus, &query, range, intervals, end_ts, &values,
            )
            .await;
            let variable = &mut state.scopes[scope].variables[index];
            variable.resolved = true;
            match result {
                Ok(values) => {
                    variable.update_options(
                        values
                            .into_iter()
                            .map(|value| crate::dashboard::variables::VariableOption {
                                text: value.clone(),
                                value,
                            })
                            .collect(),
                    );
                    if variable.query_value() != previous {
                        changed.insert(variable.name.clone());
                    }
                }
                Err(error) => variable.last_error = Some(error.to_string()),
            }
        }
        changed_by_scope[scope] = changed;
    }
}

enum PrometheusVariableQuery {
    LabelValues {
        metric: Option<String>,
        label: String,
    },
    QueryResult(String),
}

/// Why query variables are being resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VariableUpdate {
    Load,
    TimeRangeChange,
    Tick,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct VariableRefreshContext {
    pub(crate) update: VariableUpdate,
    pub(crate) range: Duration,
    pub(crate) intervals: QueryIntervals,
    pub(crate) end_ts: i64,
}

/// Variables queried in a pass and failures that retained their old values.
#[derive(Debug, Default)]
pub(crate) struct VariableReport {
    pub(crate) queried: Vec<String>,
    pub(crate) errors: Vec<(String, String)>,
}

pub(crate) async fn refresh_query_variables(
    prometheus: &prom::PromClient,
    query_vars: &[TemplateQueryVar],
    context: VariableRefreshContext,
    failed_variables: &HashSet<String>,
    vars: &mut HashMap<String, String>,
) -> VariableReport {
    let VariableRefreshContext {
        update,
        range,
        intervals,
        end_ts,
    } = context;
    let mut report = VariableReport::default();
    let mut changed = Vec::new();
    for query_var in query_vars {
        let due = match update {
            VariableUpdate::Load => true,
            VariableUpdate::TimeRangeChange => {
                query_var.refresh == crate::grafana::VariableRefresh::OnTimeRangeChange
                    || failed_variables.contains(&query_var.name)
            }
            VariableUpdate::Tick => false,
        };
        if !due && !references_any(&query_var.query, &changed) {
            continue;
        }
        report.queried.push(query_var.name.clone());
        let values = match resolve_query_variable_values(
            prometheus, query_var, range, intervals, end_ts, vars,
        )
        .await
        {
            Ok(values) => values,
            Err(error) => {
                report
                    .errors
                    .push((query_var.name.clone(), format!("{error:#}")));
                continue;
            }
        };
        let Some(value) = first_value(values) else {
            continue;
        };

        if vars.get(&query_var.name) != Some(&value) {
            changed.push(query_var.name.as_str());
        }
        vars.insert(query_var.name.clone(), value);
    }

    report
}

fn references_any(query: &str, names: &[&str]) -> bool {
    let mut found = false;
    if !names.is_empty() {
        substitute_variables(query, |name| {
            found |= names.contains(&name);
            None
        });
    }
    found
}

async fn resolve_query_variable_values(
    prometheus: &prom::PromClient,
    query_var: &TemplateQueryVar,
    range: Duration,
    intervals: QueryIntervals,
    end_ts: i64,
    vars: &HashMap<String, String>,
) -> Result<Vec<String>> {
    let expanded_query = expand_expr(&query_var.query, range, intervals, vars);
    let query = parse_prometheus_variable_query(&expanded_query)?;
    let start_ts = end_ts - range.as_secs() as i64;
    let values = match query {
        PrometheusVariableQuery::LabelValues { metric, label } => {
            prometheus
                .label_values(&label, metric.as_deref(), start_ts, end_ts)
                .await?
        }
        PrometheusVariableQuery::QueryResult(query) => {
            prometheus
                .query_instant_result_strings(&query, end_ts)
                .await?
        }
    };

    apply_regex(values, query_var.regex.as_deref())
}

fn parse_prometheus_variable_query(query: &str) -> Result<PrometheusVariableQuery> {
    if let Some(args) = call_args(query, "label_values") {
        let args = split_top_level_args(args);
        return match args.as_slice() {
            [label] => Ok(PrometheusVariableQuery::LabelValues {
                metric: None,
                label: label.trim().to_string(),
            }),
            [metric, label] => Ok(PrometheusVariableQuery::LabelValues {
                metric: Some(metric.trim().to_string()),
                label: label.trim().to_string(),
            }),
            _ => Err(anyhow!("unsupported label_values query: {}", query)),
        };
    }

    if let Some(args) = call_args(query, "query_result") {
        return Ok(PrometheusVariableQuery::QueryResult(
            args.trim().to_string(),
        ));
    }

    Ok(PrometheusVariableQuery::QueryResult(
        query.trim().to_string(),
    ))
}

fn call_args<'a>(query: &'a str, name: &str) -> Option<&'a str> {
    let query = query.trim();
    let rest = query.strip_prefix(name)?.trim_start();
    let inner = rest.strip_prefix('(')?.strip_suffix(')')?;
    Some(inner)
}

fn split_top_level_args(args: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0;

    for (idx, ch) in args.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(args[start..idx].trim());
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    parts.push(args[start..].trim());
    parts.into_iter().filter(|part| !part.is_empty()).collect()
}

fn apply_regex(values: Vec<String>, regex: Option<&str>) -> Result<Vec<String>> {
    let Some(regex) = regex.map(str::trim).filter(|regex| !regex.is_empty()) else {
        return Ok(values);
    };

    let pattern = regex
        .strip_prefix('/')
        .and_then(|regex| regex.rsplit_once('/').map(|(pattern, _)| pattern))
        .unwrap_or(regex);
    let regex = Regex::new(pattern)?;

    Ok(values
        .into_iter()
        .filter_map(|value| {
            let captures = regex.captures(&value)?;
            captures
                .get(1)
                .or_else(|| captures.get(0))
                .map(|matched| matched.as_str().to_string())
        })
        .collect())
}

fn first_value(values: Vec<String>) -> Option<String> {
    values.into_iter().find(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grafana::VariableRefresh;
    use std::sync::{Arc, Mutex};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    async fn recording_prometheus() -> (prom::PromClient, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let recorded = Arc::clone(&recorded);
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut chunk = [0_u8; 1024];
                    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                        let read = socket.read(&mut chunk).await.unwrap();
                        if read == 0 {
                            return;
                        }
                        request.extend_from_slice(&chunk[..read]);
                    }
                    let request = String::from_utf8_lossy(&request);
                    let target = request.split_whitespace().nth(1).unwrap_or("");
                    let target = urlencoding::decode(target).unwrap().into_owned();
                    recorded.lock().unwrap().push(target.clone());
                    let (status, body) = if target.starts_with("/api/v1/label/broken/") {
                        (
                            "400 Bad Request",
                            r#"{"status":"error","errorType":"bad_data","error":"invalid label"}"#,
                        )
                    } else if target.starts_with("/api/v1/label/job/") {
                        ("200 OK", r#"{"status":"success","data":["api","web"]}"#)
                    } else if target.starts_with("/api/v1/label/instance/")
                        && target.contains(r#"job="web""#)
                    {
                        ("200 OK", r#"{"status":"success","data":["web-1"]}"#)
                    } else if target.starts_with("/api/v1/label/instance/") {
                        ("200 OK", r#"{"status":"success","data":["api-1"]}"#)
                    } else {
                        ("200 OK", r#"{"status":"success","data":[]}"#)
                    };
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    socket.write_all(response.as_bytes()).await.unwrap();
                });
            }
        });
        (prom::PromClient::new(format!("http://{address}")), requests)
    }

    fn query_var(name: &str, query: &str, refresh: VariableRefresh) -> TemplateQueryVar {
        TemplateQueryVar {
            name: name.to_string(),
            query: query.to_string(),
            regex: None,
            query_path: format!("templating.list[{name}].query"),
            refresh,
        }
    }

    fn request_count(requests: &Mutex<Vec<String>>, label: &str) -> usize {
        let prefix = format!("/api/v1/label/{label}/");
        requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.starts_with(&prefix))
            .count()
    }

    fn intervals() -> QueryIntervals {
        QueryIntervals::new(
            Duration::from_secs(300),
            Duration::from_secs(15),
            Duration::from_secs(15),
            None,
        )
    }

    fn refresh_context(
        update: VariableUpdate,
        range_secs: u64,
        end_ts: i64,
    ) -> VariableRefreshContext {
        VariableRefreshContext {
            update,
            range: Duration::from_secs(range_secs),
            intervals: intervals(),
            end_ts,
        }
    }

    #[tokio::test]
    async fn refresh_modes_query_only_on_their_triggers() {
        let (client, requests) = recording_prometheus().await;
        let query_vars = vec![
            query_var("job", "label_values(job)", VariableRefresh::OnLoad),
            query_var(
                "instance",
                "label_values(instance)",
                VariableRefresh::OnTimeRangeChange,
            ),
        ];
        let mut vars = HashMap::new();

        refresh_query_variables(
            &client,
            &query_vars,
            refresh_context(VariableUpdate::Load, 300, 1000),
            &HashSet::new(),
            &mut vars,
        )
        .await;
        refresh_query_variables(
            &client,
            &query_vars,
            refresh_context(VariableUpdate::Tick, 300, 1015),
            &HashSet::new(),
            &mut vars,
        )
        .await;
        refresh_query_variables(
            &client,
            &query_vars,
            refresh_context(VariableUpdate::TimeRangeChange, 600, 1015),
            &HashSet::new(),
            &mut vars,
        )
        .await;

        assert_eq!(request_count(&requests, "job"), 1);
        assert_eq!(request_count(&requests, "instance"), 2);
    }

    #[tokio::test]
    async fn dependent_variable_refreshes_after_parent_value_changes() {
        let (client, requests) = recording_prometheus().await;
        let query_vars = vec![
            query_var(
                "job",
                "label_values(job)",
                VariableRefresh::OnTimeRangeChange,
            ),
            query_var(
                "instance",
                r#"label_values(up{job="$job"}, instance)"#,
                VariableRefresh::OnLoad,
            ),
        ];
        let mut vars = HashMap::from([("job".to_string(), "web".to_string())]);

        refresh_query_variables(
            &client,
            &query_vars,
            refresh_context(VariableUpdate::Load, 300, 1000),
            &HashSet::new(),
            &mut vars,
        )
        .await;
        vars.insert("job".to_string(), "retired".to_string());
        refresh_query_variables(
            &client,
            &query_vars,
            refresh_context(VariableUpdate::TimeRangeChange, 600, 1000),
            &HashSet::new(),
            &mut vars,
        )
        .await;
        refresh_query_variables(
            &client,
            &query_vars,
            refresh_context(VariableUpdate::TimeRangeChange, 900, 1000),
            &HashSet::new(),
            &mut vars,
        )
        .await;

        assert_eq!(request_count(&requests, "instance"), 2);
        let requests = requests.lock().unwrap();
        let instance = requests
            .iter()
            .filter(|request| request.starts_with("/api/v1/label/instance/"))
            .collect::<Vec<_>>();
        assert!(instance[0].contains(r#"match[]=up{job="api"}"#));
        assert!(instance[1].contains(r#"match[]=up{job="api"}"#));
    }

    #[tokio::test]
    async fn failed_variable_keeps_its_value_and_later_variables_resolve() {
        let (client, _) = recording_prometheus().await;
        let query_vars = vec![
            query_var(
                "broken",
                "label_values(broken)",
                VariableRefresh::OnTimeRangeChange,
            ),
            query_var("job", "label_values(job)", VariableRefresh::OnLoad),
        ];
        let mut vars = HashMap::from([("broken".to_string(), "saved".to_string())]);

        let report = refresh_query_variables(
            &client,
            &query_vars,
            refresh_context(VariableUpdate::Load, 300, 1000),
            &HashSet::new(),
            &mut vars,
        )
        .await;

        assert_eq!(vars["broken"], "saved");
        assert_eq!(vars["job"], "api");
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].0, "broken");
        assert!(report.errors[0].1.contains("bad_data: invalid label"));
    }

    #[tokio::test]
    async fn scoped_load_variables_run_once_and_newly_active_scopes_load_on_tick() {
        use crate::dashboard::variables::{Variable, VariableScope, VariableState};
        let (client, requests) = recording_prometheus().await;
        let variable = |name: &str| Variable {
            name: name.into(),
            values: Vec::new(),
            texts: Vec::new(),
            options: Vec::new(),
            all: false,
            all_value: None,
            repeatable: true,
            last_error: None,
            resolved: false,
            query: Some(query_var(
                name,
                &format!("label_values({name})"),
                VariableRefresh::OnLoad,
            )),
        };
        let mut state = VariableState {
            scopes: vec![
                VariableScope {
                    parent: None,
                    variables: vec![variable("root")],
                },
                VariableScope {
                    parent: Some(0),
                    variables: vec![variable("child")],
                },
            ],
            ..Default::default()
        };

        refresh_scoped_variables(
            &client,
            &mut state,
            &HashSet::from([0]),
            VariableRefreshContext {
                update: VariableUpdate::Load,
                range: Duration::from_secs(300),
                intervals: intervals(),
                end_ts: 1000,
            },
            &HashMap::new(),
        )
        .await;
        refresh_scoped_variables(
            &client,
            &mut state,
            &HashSet::from([0]),
            VariableRefreshContext {
                update: VariableUpdate::Tick,
                range: Duration::from_secs(300),
                intervals: intervals(),
                end_ts: 1015,
            },
            &HashMap::new(),
        )
        .await;
        refresh_scoped_variables(
            &client,
            &mut state,
            &HashSet::from([0, 1]),
            VariableRefreshContext {
                update: VariableUpdate::Tick,
                range: Duration::from_secs(300),
                intervals: intervals(),
                end_ts: 1030,
            },
            &HashMap::new(),
        )
        .await;

        assert_eq!(request_count(&requests, "root"), 1);
        assert_eq!(request_count(&requests, "child"), 1);
    }

    #[tokio::test]
    async fn scoped_all_option_change_refreshes_dependent_variable() {
        use crate::dashboard::variables::{Variable, VariableOption, VariableScope, VariableState};
        let (client, requests) = recording_prometheus().await;
        let root = Variable {
            name: "job".into(),
            values: vec!["$__all".into()],
            texts: vec!["All".into()],
            options: vec![VariableOption {
                value: "api".into(),
                text: "api".into(),
            }],
            all: true,
            all_value: None,
            repeatable: true,
            query: Some(query_var(
                "job",
                "label_values(job)",
                VariableRefresh::OnTimeRangeChange,
            )),
            last_error: None,
            resolved: true,
        };
        let dependent = Variable {
            name: "instance".into(),
            values: vec!["api-1".into()],
            texts: vec!["api-1".into()],
            options: vec![VariableOption {
                value: "api-1".into(),
                text: "api-1".into(),
            }],
            all: false,
            all_value: None,
            repeatable: true,
            query: Some(query_var(
                "instance",
                r#"label_values(up{job=~"$job"}, instance)"#,
                VariableRefresh::OnLoad,
            )),
            last_error: None,
            resolved: true,
        };
        let mut state = VariableState {
            scopes: vec![VariableScope {
                parent: None,
                variables: vec![root, dependent],
            }],
            ..Default::default()
        };

        refresh_scoped_variables(
            &client,
            &mut state,
            &HashSet::from([0]),
            VariableRefreshContext {
                update: VariableUpdate::TimeRangeChange,
                range: Duration::from_secs(300),
                intervals: intervals(),
                end_ts: 1000,
            },
            &HashMap::new(),
        )
        .await;

        assert_eq!(state.lookup(0, "job").unwrap().query_value(), "(api|web)");
        assert_eq!(request_count(&requests, "instance"), 1);
    }

    #[tokio::test]
    async fn scoped_refresh_retains_selection_and_continues_after_error() {
        use crate::dashboard::variables::{Variable, VariableOption, VariableScope, VariableState};
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = prom::PromClient::new(format!("http://{}", listener.local_addr().unwrap()));
        let server = tokio::spawn(async move {
            for body in [
                r#"{"status":"error","errorType":"bad_data","error":"broken"}"#,
                r#"{"status":"success","data":["node-a","node-b","node-c"]}"#,
                r#"{"status":"success","data":["node-c"]}"#,
            ] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = [0; 4096];
                let _ = socket.read(&mut buf).await.unwrap();
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
            }
        });
        let variable = |name: &str| Variable {
            name: name.into(),
            values: vec!["node-b".into()],
            texts: vec!["B".into()],
            options: vec![VariableOption {
                value: "node-b".into(),
                text: "B".into(),
            }],
            all: false,
            all_value: None,
            repeatable: true,
            last_error: None,
            resolved: false,
            query: Some(TemplateQueryVar {
                name: name.into(),
                query: "label_values(instance)".into(),
                regex: None,
                query_path: "spec.variables".into(),
                refresh: Default::default(),
            }),
        };
        let mut state = VariableState {
            scopes: vec![VariableScope {
                parent: None,
                variables: vec![variable("broken"), variable("node")],
            }],
            ..Default::default()
        };
        tokio::time::timeout(
            Duration::from_secs(3),
            refresh_scoped_variables(
                &client,
                &mut state,
                &HashSet::from([0]),
                VariableRefreshContext {
                    update: VariableUpdate::Load,
                    range: Duration::from_secs(300),
                    intervals: QueryIntervals::new(
                        Duration::from_secs(300),
                        Duration::from_secs(15),
                        Duration::from_secs(15),
                        None,
                    ),
                    end_ts: 1000,
                },
                &HashMap::new(),
            ),
        )
        .await
        .unwrap();
        let broken = state.lookup(0, "broken").unwrap();
        assert!(broken.last_error.is_some());
        assert_eq!(broken.values, vec!["node-b"]);
        let node = state.lookup(0, "node").unwrap();
        assert_eq!(node.values, vec!["node-b"]);
        assert_eq!(node.texts, vec!["node-b"]);
        assert_eq!(
            node.options
                .iter()
                .map(|o| o.value.as_str())
                .collect::<Vec<_>>(),
            vec!["node-a", "node-b", "node-c"]
        );

        refresh_scoped_variables(
            &client,
            &mut state,
            &HashSet::from([0]),
            VariableRefreshContext {
                update: VariableUpdate::TimeRangeChange,
                range: Duration::from_secs(300),
                intervals: intervals(),
                end_ts: 1015,
            },
            &HashMap::new(),
        )
        .await;
        let recovered = state.lookup(0, "broken").unwrap();
        assert_eq!(recovered.values, vec!["node-c"]);
        assert!(recovered.last_error.is_none());
        server.await.unwrap();
    }
    #[test]
    fn test_split_label_values_query() {
        let PrometheusVariableQuery::LabelValues { metric, label } =
            parse_prometheus_variable_query("label_values(up{job=~\"$job\"}, instance)").unwrap()
        else {
            panic!("expected label_values query");
        };

        assert_eq!(metric.as_deref(), Some("up{job=~\"$job\"}"));
        assert_eq!(label, "instance");
    }

    #[test]
    fn test_parse_label_values_without_metric() {
        let PrometheusVariableQuery::LabelValues { metric, label } =
            parse_prometheus_variable_query("label_values(model_name)").unwrap()
        else {
            panic!("expected label_values query");
        };

        assert!(metric.is_none());
        assert_eq!(label, "model_name");
    }

    #[test]
    fn test_regex_extracts_first_capture_group() {
        let values = vec![
            r#"{instance="node-2", job="node"} 1"#.to_string(),
            r#"{instance="node-1", job="node"} 1"#.to_string(),
        ];

        let values = apply_regex(values, Some(r#"/instance="([^"]+)"/"#)).unwrap();

        assert_eq!(first_value(values), Some("node-2".to_string()));
    }
}
