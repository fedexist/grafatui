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

use super::data::expand_expr;
use crate::grafana::TemplateQueryVar;
use crate::prom;
use anyhow::{Result, anyhow};
use regex::Regex;
use std::collections::HashMap;
use std::time::Duration;

enum PrometheusVariableQuery {
    LabelValues {
        metric: Option<String>,
        label: String,
    },
    QueryResult(String),
}

pub(crate) async fn refresh_query_variables(
    prometheus: &prom::PromClient,
    query_vars: &[TemplateQueryVar],
    range: Duration,
    step: Duration,
    end_ts: i64,
    vars: &mut HashMap<String, String>,
) -> Result<()> {
    for query_var in query_vars {
        let Some(value) =
            resolve_query_variable(prometheus, query_var, range, step, end_ts, vars).await?
        else {
            continue;
        };

        vars.insert(query_var.name.clone(), value);
    }

    Ok(())
}

async fn resolve_query_variable(
    prometheus: &prom::PromClient,
    query_var: &TemplateQueryVar,
    range: Duration,
    step: Duration,
    end_ts: i64,
    vars: &HashMap<String, String>,
) -> Result<Option<String>> {
    let expanded_query = expand_expr(&query_var.query, range, step, vars);
    let query = parse_prometheus_variable_query(&expanded_query)?;
    let start_ts = end_ts - range.as_secs() as i64;
    let values = match query {
        PrometheusVariableQuery::LabelValues { metric, label } => {
            if let Some(metric) = metric {
                prometheus
                    .series_label_values(&metric, &label, start_ts, end_ts)
                    .await?
            } else {
                prometheus.label_values(&label).await?
            }
        }
        PrometheusVariableQuery::QueryResult(query) => {
            prometheus
                .query_instant_result_strings(&query, end_ts)
                .await?
        }
    };

    Ok(first_value(apply_regex(
        values,
        query_var.regex.as_deref(),
    )?))
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
