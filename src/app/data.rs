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

const PANEL_RESOLUTION_POINTS: u32 = 200;

pub(crate) fn expand_expr(
    expr: &str,
    range: Duration,
    step: Duration,
    vars: &HashMap<String, String>,
) -> String {
    let mut s = expr.to_string();

    let interval = interval_duration(range, step);
    s = replace_builtin(&s, "__interval_ms", &interval.as_millis().to_string());
    s = replace_builtin(&s, "__interval", &format_prom_duration(interval));
    s = replace_builtin(&s, "__range_ms", &range.as_millis().to_string());
    s = replace_builtin(&s, "__range_s", &range.as_secs().to_string());
    s = replace_builtin(&s, "__range", &format_prom_duration(range));

    let interval_secs = std::cmp::max(step.as_secs() * 4, 60);
    let interval_param = format!("{}s", interval_secs);
    s = replace_builtin(
        &s,
        "__rate_interval_ms",
        &(interval_secs * 1000).to_string(),
    );
    s = replace_builtin(&s, "__rate_interval", &interval_param);

    for (k, v) in vars {
        s = s.replace(&format!("${{{}}}", k), v);
        s = s.replace(&format!("${}", k), v);
    }

    s
}

fn replace_builtin(expr: &str, name: &str, value: &str) -> String {
    expr.replace(&format!("${{{}}}", name), value)
        .replace(&format!("${}", name), value)
}

fn interval_duration(range: Duration, step: Duration) -> Duration {
    let resolution = range / PANEL_RESOLUTION_POINTS;
    let interval = resolution.max(step);
    interval.max(Duration::from_secs(1))
}

fn format_prom_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs == 0 {
        return format!("{}ms", duration.as_millis().max(1));
    }

    const DAY: u64 = 24 * 60 * 60;
    const HOUR: u64 = 60 * 60;
    const MINUTE: u64 = 60;

    if secs % DAY == 0 {
        format!("{}d", secs / DAY)
    } else if secs % HOUR == 0 {
        format!("{}h", secs / HOUR)
    } else if secs % MINUTE == 0 {
        format!("{}m", secs / MINUTE)
    } else {
        format!("{}s", secs)
    }
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

pub(crate) fn default_queries(mut provided: Vec<String>) -> Vec<PanelState> {
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

pub(crate) fn parse_duration(s: &str) -> Result<Duration> {
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
        let expanded = expand_expr(expr, Duration::from_secs(300), step, &vars);
        assert_eq!(expanded, "rate(http_requests_total[60s])");

        let step = Duration::from_secs(30);
        let expr = "rate(http_requests_total[$__rate_interval])";
        let expanded = expand_expr(expr, Duration::from_secs(300), step, &vars);
        assert_eq!(expanded, "rate(http_requests_total[120s])");
    }

    #[test]
    fn test_expand_expr_vars() {
        let mut vars = HashMap::new();
        vars.insert("job".to_string(), "node-exporter".to_string());
        vars.insert("instance".to_string(), "localhost:9100".to_string());

        let step = Duration::from_secs(15);

        let expr = "up{job=\"$job\"}";
        let expanded = expand_expr(expr, Duration::from_secs(300), step, &vars);
        assert_eq!(expanded, "up{job=\"node-exporter\"}");

        let expr = "up{instance=\"${instance}\"}";
        let expanded = expand_expr(expr, Duration::from_secs(300), step, &vars);
        assert_eq!(expanded, "up{instance=\"localhost:9100\"}");

        let expr =
            "rate(http_requests_total{job=\"$job\", instance=\"$instance\"}[$__rate_interval])";
        let expanded = expand_expr(expr, Duration::from_secs(300), step, &vars);
        assert_eq!(
            expanded,
            "rate(http_requests_total{job=\"node-exporter\", instance=\"localhost:9100\"}[60s])"
        );
    }

    #[test]
    fn test_expand_expr_builtin_intervals_and_range() {
        let vars = HashMap::new();
        let range = Duration::from_secs(24 * 60 * 60);
        let step = Duration::from_secs(60);
        let expr = "rate(http_requests_total[$__interval]) offset $__range";
        let expanded = expand_expr(expr, range, step, &vars);
        assert_eq!(expanded, "rate(http_requests_total[432s]) offset 1d");

        let expr = "sum_over_time(up[${__range_s}s]) / $__interval_ms / $__range_ms";
        let expanded = expand_expr(expr, range, step, &vars);
        assert_eq!(expanded, "sum_over_time(up[86400s]) / 432000 / 86400000");
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
