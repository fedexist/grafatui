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

use super::state::{GraphOptions, PanelOptions, PanelState, PanelType, YAxisMode};
use anyhow::Result;
use std::borrow::Cow;
use std::collections::HashMap;
use std::time::Duration;

/// Grafana's default Prometheus scrape interval, used for `$__rate_interval`.
pub(crate) const DEFAULT_SCRAPE_INTERVAL: Duration = Duration::from_secs(15);
pub(crate) const DEFAULT_MIN_STEP: Duration = Duration::from_secs(5);

/// How Grafatui chooses the range-query step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepPolicy {
    /// A CLI/configured step is an exact user request.
    Explicit(Duration),
    /// With no configured step, scale from this compatibility minimum.
    Automatic { min_step: Duration },
}

impl From<Duration> for StepPolicy {
    fn from(step: Duration) -> Self {
        Self::Explicit(step)
    }
}

pub(crate) fn resolve_step_policy(cli: Option<&str>, config: Option<&str>) -> Result<StepPolicy> {
    let Some(text) = cli.or(config) else {
        return Ok(StepPolicy::Automatic {
            min_step: DEFAULT_MIN_STEP,
        });
    };
    let step = humantime::parse_duration(text)?;
    anyhow::ensure!(!step.is_zero(), "step must be greater than zero");
    Ok(StepPolicy::Explicit(step))
}

/// Points a range query asks for when its panel sets no `maxDataPoints`; about
/// the pixel width Grafana uses for a typical panel.
const DEFAULT_MAX_DATA_POINTS: u32 = 1000;

/// Prometheus rejects range queries that would return more points per series.
const PROMETHEUS_MAX_POINTS: u64 = 11_000;

/// The step and `$__rate_interval` one query runs with, derived from the time
/// range the way Grafana's Prometheus datasource does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueryIntervals {
    /// Range query step, also substituted for `$__interval`.
    pub(crate) step: Duration,
    /// Substituted for `$__rate_interval`.
    pub(crate) rate_interval: Duration,
}

impl QueryIntervals {
    pub(crate) fn for_policy(
        range: Duration,
        policy: StepPolicy,
        scrape_interval: Duration,
        min_interval: Option<Duration>,
        max_data_points: Option<u32>,
    ) -> Self {
        match policy {
            StepPolicy::Explicit(step) => Self::exact(step, scrape_interval),
            StepPolicy::Automatic { min_step } => {
                let query_scrape_interval = min_interval.unwrap_or(scrape_interval);
                let min_interval = min_interval.unwrap_or(min_step);
                Self::new(range, min_interval, query_scrape_interval, max_data_points)
            }
        }
    }

    fn exact(step: Duration, scrape_interval: Duration) -> Self {
        Self {
            step,
            rate_interval: (step + scrape_interval).max(scrape_interval * 4),
        }
    }

    /// Divides `range` by `max_data_points` (default 1000), rounds the result
    /// to a Grafana-style interval, and keeps it no finer than `min_interval`.
    /// The step is coarsened further when Prometheus's 11,000-point limit
    /// would be exceeded.
    pub(crate) fn new(
        range: Duration,
        min_interval: Duration,
        scrape_interval: Duration,
        max_data_points: Option<u32>,
    ) -> Self {
        let points = max_data_points
            .filter(|points| *points > 0)
            .unwrap_or(DEFAULT_MAX_DATA_POINTS);
        let raw = range / points;
        let interval = if raw < min_interval {
            min_interval
        } else {
            round_interval(raw).max(min_interval)
        };
        let safe = Duration::from_secs(range.as_secs().div_ceil(PROMETHEUS_MAX_POINTS));
        let step = interval.max(safe).max(Duration::from_secs(1));
        let rate_interval = (step + scrape_interval).max(scrape_interval * 4);
        Self {
            step,
            rate_interval,
        }
    }
}

/// Rounds an interval to the nearest of Grafana's preferred intervals
/// (`kbn.roundInterval`), so steps read as 1m or 5m rather than 1m26s.
fn round_interval(interval: Duration) -> Duration {
    const SECOND: u64 = 1000;
    const MINUTE: u64 = 60 * SECOND;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    // (upper bound, rounded value), both in milliseconds.
    const TABLE: &[(u64, u64)] = &[
        (10, 1),
        (15, 10),
        (35, 20),
        (75, 50),
        (150, 100),
        (350, 200),
        (750, 500),
        (1_500, SECOND),
        (3_500, 2 * SECOND),
        (7_500, 5 * SECOND),
        (12_500, 10 * SECOND),
        (17_500, 15 * SECOND),
        (25_000, 20 * SECOND),
        (45_000, 30 * SECOND),
        (90_000, MINUTE),
        (210_000, 2 * MINUTE),
        (450_000, 5 * MINUTE),
        (750_000, 10 * MINUTE),
        (1_050_000, 15 * MINUTE),
        (1_500_000, 20 * MINUTE),
        (2_700_000, 30 * MINUTE),
        (5_400_000, HOUR),
        (9_000_000, 2 * HOUR),
        (16_200_000, 3 * HOUR),
        (32_400_000, 6 * HOUR),
        (86_400_000, 12 * HOUR),
        (604_800_000, DAY),
        (1_814_400_000, 7 * DAY),
        (3_628_800_000, 30 * DAY),
    ];
    let ms = interval.as_millis() as u64;
    let rounded = TABLE
        .iter()
        .find(|(bound, _)| ms <= *bound)
        .map_or(365 * DAY, |(_, value)| *value);
    Duration::from_millis(rounded)
}

/// Parses a Grafana min interval such as `30s`, `1m`, or `>10s`.
pub(crate) fn parse_min_interval(text: &str) -> Option<Duration> {
    let text = text.trim();
    let text = text.strip_prefix('>').unwrap_or(text).trim();
    if text.is_empty() {
        return None;
    }
    humantime::parse_duration(text)
        .ok()
        .filter(|interval| !interval.is_zero())
}

pub(crate) fn expand_expr(
    expr: &str,
    range: Duration,
    intervals: QueryIntervals,
    vars: &HashMap<String, String>,
) -> String {
    let interval = intervals.step;
    let rate_interval_secs = intervals.rate_interval.as_secs();
    super::variables::substitute_variables(expr, |name| {
        Some(match name {
            "__interval" => Cow::Owned(format_prom_duration(interval)),
            "__interval_ms" => Cow::Owned(interval.as_millis().to_string()),
            "__range" => Cow::Owned(format_prom_duration(range)),
            "__range_s" => Cow::Owned(range.as_secs().to_string()),
            "__range_ms" => Cow::Owned(range.as_millis().to_string()),
            "__rate_interval" => Cow::Owned(format!("{rate_interval_secs}s")),
            "__rate_interval_ms" => Cow::Owned((rate_interval_secs * 1000).to_string()),
            _ => Cow::Borrowed(vars.get(name)?.as_str()),
        })
    })
}

fn format_prom_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs == 0 {
        return format!("{}ms", duration.as_millis().max(1));
    }

    const DAY: u64 = 24 * 60 * 60;
    const HOUR: u64 = 60 * 60;
    const MINUTE: u64 = 60;

    if secs.is_multiple_of(DAY) {
        format!("{}d", secs / DAY)
    } else if secs.is_multiple_of(HOUR) {
        format!("{}h", secs / HOUR)
    } else if secs.is_multiple_of(MINUTE) {
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
            query_modes: vec![crate::app::QueryMode::Range],
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
            display: crate::ui::DisplayFormat::default(),
            options: PanelOptions::Graph(GraphOptions::default()),
            resolution: Default::default(),
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

    const SECOND: Duration = Duration::from_secs(1);
    const MINUTE: Duration = Duration::from_secs(60);
    const HOUR: Duration = Duration::from_secs(60 * 60);
    const DAY: Duration = Duration::from_secs(24 * 60 * 60);

    fn intervals(range: Duration, min_step: Duration) -> QueryIntervals {
        QueryIntervals::new(range, min_step, DEFAULT_SCRAPE_INTERVAL, None)
    }

    #[test]
    fn resolve_step_policy_prefers_cli_then_config() {
        assert_eq!(
            resolve_step_policy(Some("17s"), Some("23s")).unwrap(),
            StepPolicy::Explicit(17 * SECOND)
        );
        assert_eq!(
            resolve_step_policy(None, Some("23s")).unwrap(),
            StepPolicy::Explicit(23 * SECOND)
        );
    }

    #[test]
    fn resolve_step_policy_is_automatic_when_absent() {
        assert_eq!(
            resolve_step_policy(None, None).unwrap(),
            StepPolicy::Automatic {
                min_step: 5 * SECOND
            }
        );
    }

    #[test]
    fn explicit_step_is_not_coarsened_past_point_limit() {
        let intervals = QueryIntervals::for_policy(
            7 * DAY,
            StepPolicy::Explicit(5 * SECOND),
            DEFAULT_SCRAPE_INTERVAL,
            Some(MINUTE),
            Some(10),
        );
        assert_eq!(intervals.step, 5 * SECOND);
    }

    #[test]
    fn automatic_step_stays_within_point_limit() {
        let intervals = QueryIntervals::for_policy(
            7 * DAY,
            StepPolicy::Automatic { min_step: SECOND },
            DEFAULT_SCRAPE_INTERVAL,
            None,
            Some(u32::MAX),
        );
        assert!((7 * DAY).as_secs() / intervals.step.as_secs() <= PROMETHEUS_MAX_POINTS);
    }

    #[test]
    fn short_ranges_use_the_minimum_step() {
        assert_eq!(intervals(5 * MINUTE, 5 * SECOND).step, 5 * SECOND);
        assert_eq!(intervals(HOUR, 5 * SECOND).step, 5 * SECOND);
    }

    #[test]
    fn long_ranges_use_a_rounded_coarser_step() {
        // 6h / 1000 = 21.6s, rounded to 20s.
        assert_eq!(intervals(6 * HOUR, 5 * SECOND).step, 20 * SECOND);
        // 24h / 1000 = 86.4s, rounded to 1m.
        assert_eq!(intervals(DAY, 5 * SECOND).step, MINUTE);
        // 7d / 1000 = 604.8s, rounded to 10m.
        assert_eq!(intervals(7 * DAY, 5 * SECOND).step, 10 * MINUTE);
    }

    #[test]
    fn steps_stay_within_the_prometheus_point_limit() {
        for range in [HOUR, 16 * HOUR, DAY, 7 * DAY, 30 * DAY, 365 * DAY] {
            for points in [None, Some(1), Some(50_000), Some(u32::MAX)] {
                let step = QueryIntervals::new(range, SECOND, DEFAULT_SCRAPE_INTERVAL, points).step;
                assert!(
                    range.as_secs() / step.as_secs() <= PROMETHEUS_MAX_POINTS,
                    "{range:?} with {points:?} points gave step {step:?}"
                );
            }
        }
    }

    #[test]
    fn max_data_points_and_min_interval_set_the_step() {
        // 1h / 60 points = 60s.
        let step = QueryIntervals::new(HOUR, SECOND, DEFAULT_SCRAPE_INTERVAL, Some(60)).step;
        assert_eq!(step, MINUTE);
        // A min interval above the computed interval wins, unrounded.
        assert_eq!(intervals(HOUR, 45 * SECOND).step, 45 * SECOND);
        // Zero max data points falls back to the default.
        let step = QueryIntervals::new(DAY, SECOND, DEFAULT_SCRAPE_INTERVAL, Some(0)).step;
        assert_eq!(step, MINUTE);
    }

    #[test]
    fn rate_interval_follows_grafana() {
        // max($__interval + scrape, 4 * scrape)
        let short = QueryIntervals::new(5 * MINUTE, 5 * SECOND, 15 * SECOND, None);
        assert_eq!(short.rate_interval, MINUTE);
        let long = QueryIntervals::new(DAY, 5 * SECOND, 15 * SECOND, None);
        assert_eq!(long.rate_interval, 75 * SECOND);
        let slow_scrape = QueryIntervals::new(5 * MINUTE, 5 * SECOND, MINUTE, None);
        assert_eq!(slow_scrape.rate_interval, 4 * MINUTE);
    }

    #[test]
    fn parses_grafana_min_intervals() {
        assert_eq!(parse_min_interval("30s"), Some(30 * SECOND));
        assert_eq!(parse_min_interval(">1m"), Some(MINUTE));
        assert_eq!(parse_min_interval(" > 2h "), Some(2 * HOUR));
        assert_eq!(parse_min_interval(""), None);
        assert_eq!(parse_min_interval("0s"), None);
        assert_eq!(parse_min_interval("$interval"), None);
    }

    #[test]
    fn scoped_expansion_respects_complete_variable_names() {
        let vars = HashMap::from([("node".into(), "a".into()), ("node_id".into(), "b".into())]);
        assert_eq!(
            expand_expr(
                "$node_id / ${node_id} / $node / $node_other",
                Duration::from_secs(300),
                intervals(Duration::from_secs(300), Duration::from_secs(15)),
                &vars
            ),
            "b / b / a / $node_other"
        );
    }
    #[test]
    fn test_expand_expr_rate_interval() {
        let vars = HashMap::new();
        let expr = "rate(http_requests_total[$__rate_interval])";
        let expanded = expand_expr(expr, 5 * MINUTE, intervals(5 * MINUTE, 15 * SECOND), &vars);
        assert_eq!(expanded, "rate(http_requests_total[60s])");

        let slow_scrape = QueryIntervals::new(5 * MINUTE, 30 * SECOND, 30 * SECOND, None);
        let expanded = expand_expr(expr, 5 * MINUTE, slow_scrape, &vars);
        assert_eq!(expanded, "rate(http_requests_total[120s])");
    }

    #[test]
    fn test_expand_expr_vars() {
        let mut vars = HashMap::new();
        vars.insert("job".to_string(), "node-exporter".to_string());
        vars.insert("instance".to_string(), "localhost:9100".to_string());

        let range = 5 * MINUTE;
        let intervals = intervals(range, 15 * SECOND);

        let expr = "up{job=\"$job\"}";
        let expanded = expand_expr(expr, range, intervals, &vars);
        assert_eq!(expanded, "up{job=\"node-exporter\"}");

        let expr = "up{instance=\"${instance}\"}";
        let expanded = expand_expr(expr, range, intervals, &vars);
        assert_eq!(expanded, "up{instance=\"localhost:9100\"}");

        let expr =
            "rate(http_requests_total{job=\"$job\", instance=\"$instance\"}[$__rate_interval])";
        let expanded = expand_expr(expr, range, intervals, &vars);
        assert_eq!(
            expanded,
            "rate(http_requests_total{job=\"node-exporter\", instance=\"localhost:9100\"}[60s])"
        );
    }

    #[test]
    fn test_expand_expr_builtin_intervals_and_range() {
        let vars = HashMap::new();
        let range = DAY;
        let intervals = intervals(range, MINUTE);
        let expr = "rate(http_requests_total[$__interval]) offset $__range";
        let expanded = expand_expr(expr, range, intervals, &vars);
        assert_eq!(expanded, "rate(http_requests_total[1m]) offset 1d");

        let expr = "sum_over_time(up[${__range_s}s]) / $__interval_ms / $__range_ms";
        let expanded = expand_expr(expr, range, intervals, &vars);
        assert_eq!(expanded, "sum_over_time(up[86400s]) / 60000 / 86400000");
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
