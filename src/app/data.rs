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

use super::state::{PanelState, PanelType, YAxisMode};
use anyhow::Result;
use std::collections::HashMap;
use std::time::Duration;

pub(crate) fn expand_expr(expr: &str, step: Duration, vars: &HashMap<String, String>) -> String {
    let mut s = expr.to_string();

    let interval_secs = std::cmp::max(step.as_secs() * 4, 60);
    let interval_param = format!("{}s", interval_secs);
    s = s.replace("$__rate_interval", &interval_param);

    for (k, v) in vars {
        s = s.replace(&format!("${{{}}}", k), v);
        s = s.replace(&format!("${}", k), v);
    }

    s
}

pub(crate) fn format_legend(fmt: &str, metric: &HashMap<String, String>) -> String {
    let mut out = fmt.to_string();
    for (k, v) in metric {
        out = out.replace(&format!("{{{{{}}}}}", k), v);
    }
    out
}

/// Downsamples data points to a maximum number of points using max-pooling.
/// This preserves peaks which is important for metrics.
pub(crate) fn downsample(points: Vec<(f64, f64)>, max_points: usize) -> Vec<(f64, f64)> {
    if points.len() <= max_points {
        return points;
    }

    let chunk_size = (points.len() as f64 / max_points as f64).ceil() as usize;
    if chunk_size <= 1 {
        return points;
    }

    points
        .chunks(chunk_size)
        .filter_map(|chunk| {
            chunk
                .iter()
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .cloned()
        })
        .collect()
}

pub fn default_queries(mut provided: Vec<String>) -> Vec<PanelState> {
    if provided.is_empty() {
        provided = vec![
            r#"sum(rate(http_requests_total{job!="prometheus"}[5m]))"#.to_string(),
            r#"sum by (instance) (process_cpu_seconds_total)"#.to_string(),
            r#"up"#.to_string(),
        ];
    }
    provided
        .into_iter()
        .map(|q| PanelState {
            title: q.clone(),
            exprs: vec![q],
            legends: vec![None],
            series: vec![],
            last_error: None,
            last_url: None,
            last_samples: 0,
            grid: None,
            y_axis_mode: YAxisMode::Auto,
            panel_type: PanelType::Graph,
            thresholds: None,
            min: None,
            max: None,
            autogrid: None,
        })
        .collect()
}

pub fn parse_duration(s: &str) -> Result<Duration> {
    Ok(humantime::parse_duration(s)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::Duration;

    #[test]
    fn test_expand_expr_rate_interval() {
        let vars = HashMap::new();
        let step = Duration::from_secs(15);
        let expr = "rate(http_requests_total[$__rate_interval])";
        let expanded = expand_expr(expr, step, &vars);
        assert_eq!(expanded, "rate(http_requests_total[60s])");

        let step = Duration::from_secs(30);
        let expr = "rate(http_requests_total[$__rate_interval])";
        let expanded = expand_expr(expr, step, &vars);
        assert_eq!(expanded, "rate(http_requests_total[120s])");
    }

    #[test]
    fn test_expand_expr_vars() {
        let mut vars = HashMap::new();
        vars.insert("job".to_string(), "node-exporter".to_string());
        vars.insert("instance".to_string(), "localhost:9100".to_string());

        let step = Duration::from_secs(15);

        let expr = "up{job=\"$job\"}";
        let expanded = expand_expr(expr, step, &vars);
        assert_eq!(expanded, "up{job=\"node-exporter\"}");

        let expr = "up{instance=\"${instance}\"}";
        let expanded = expand_expr(expr, step, &vars);
        assert_eq!(expanded, "up{instance=\"localhost:9100\"}");

        let expr =
            "rate(http_requests_total{job=\"$job\", instance=\"$instance\"}[$__rate_interval])";
        let expanded = expand_expr(expr, step, &vars);
        assert_eq!(
            expanded,
            "rate(http_requests_total{job=\"node-exporter\", instance=\"localhost:9100\"}[60s])"
        );
    }

    #[test]
    fn test_format_legend() {
        let mut metric = HashMap::new();
        metric.insert("job".to_string(), "node".to_string());
        metric.insert("instance".to_string(), "localhost".to_string());

        let fmt = "Job: {{job}} - {{instance}}";
        assert_eq!(format_legend(fmt, &metric), "Job: node - localhost");

        let fmt2 = "Static Text";
        assert_eq!(format_legend(fmt2, &metric), "Static Text");
    }

    #[test]
    fn test_downsample() {
        let points: Vec<(f64, f64)> = (0..1000).map(|i| (i as f64, i as f64)).collect();
        let downsampled = downsample(points, 100);
        assert_eq!(downsampled.len(), 100);
        assert_eq!(downsampled.last().unwrap().1, 999.0);
    }
}
