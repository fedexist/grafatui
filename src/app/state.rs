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

use crate::app::data::{downsample, expand_expr, format_legend};
use crate::app::variables::refresh_query_variables;
use crate::grafana::TemplateQueryVar;
use crate::prom;
use crate::theme::Theme;
use anyhow::Result;
use futures::StreamExt;
use ratatui::style::Color;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Represents the state of a single dashboard panel.
#[derive(Debug, Clone)]
pub(crate) struct PanelState {
    /// Panel title.
    pub(crate) title: String,
    /// PromQL expressions to query.
    pub(crate) exprs: Vec<String>,
    /// Optional legend formats (e.g. "{{instance}}"). Parallel to exprs.
    pub(crate) legends: Vec<Option<String>>,
    /// Current time-series data for this panel.
    pub(crate) series: Vec<SeriesView>,
    /// Last error message, if any.
    pub(crate) last_error: Option<String>,
    /// Last query URL used (for debugging).
    pub(crate) last_url: Option<String>,
    /// Total number of samples in the current view.
    pub(crate) last_samples: usize,
    /// Grid layout position (if imported from Grafana).
    pub(crate) grid: Option<GridUnit>,
    /// Y-axis scaling mode.
    pub(crate) y_axis_mode: YAxisMode,
    /// Visualization type.
    pub(crate) panel_type: PanelType,
    /// Threshold configuration.
    pub(crate) thresholds: Option<Thresholds>,
    /// Optional minimum value for gauge and thresholds.
    pub(crate) min: Option<f64>,
    /// Optional maximum value for gauge and thresholds.
    pub(crate) max: Option<f64>,
    /// Whether to render automatic grid lines for this panel.
    pub(crate) autogrid: Option<bool>,
}

/// Visualization types supported by Grafatui.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PanelType {
    Graph,
    Gauge,
    BarGauge,
    Table,
    Stat,
    Heatmap,
    Unknown,
}

/// Modes for Y-axis scaling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum YAxisMode {
    /// Auto-scale based on min/max of data.
    Auto,
    /// Always include zero.
    ZeroBased,
}

/// Represents a single time-series line in a chart.
#[derive(Debug, Clone)]
pub(crate) struct SeriesView {
    /// Stable name of the series (used for coloring).
    pub(crate) name: String,
    /// Latest value of the series (used for display).
    pub(crate) value: Option<f64>,
    /// Data points (timestamp, value).
    pub(crate) points: Vec<(f64, f64)>,
    /// Whether the series is visible in the chart.
    pub(crate) visible: bool,
}

/// Grid positioning unit (Grafana style).
#[derive(Debug, Clone, Copy)]
pub(crate) struct GridUnit {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ThresholdMode {
    Absolute,
    Percentage,
}

#[derive(Debug, Clone)]
pub(crate) struct ThresholdStep {
    pub(crate) value: Option<f64>,
    pub(crate) color: Color,
}

#[derive(Debug, Clone)]
pub(crate) struct Thresholds {
    pub(crate) mode: ThresholdMode,
    pub(crate) steps: Vec<ThresholdStep>,
    pub(crate) style: Option<String>,
}

impl PanelState {
    pub(crate) fn get_color_for_value(&self, val: f64) -> Option<Color> {
        let thresholds = self.thresholds.as_ref()?;

        let mut matched_color = None;

        match thresholds.mode {
            ThresholdMode::Absolute => {
                for step in &thresholds.steps {
                    if let Some(step_val) = step.value {
                        if val >= step_val {
                            matched_color = Some(step.color);
                        }
                    } else {
                        // Null value represents the base step (lowest possible)
                        if matched_color.is_none() {
                            matched_color = Some(step.color);
                        }
                    }
                }
            }
            ThresholdMode::Percentage => {
                let min = self.min.unwrap_or(0.0);
                let max = self.max.unwrap_or(100.0);
                let range = max - min;

                let pct = if range > 0.0 {
                    (val - min) / range * 100.0
                } else {
                    0.0
                };

                for step in &thresholds.steps {
                    if let Some(step_val) = step.value {
                        if pct >= step_val {
                            matched_color = Some(step.color);
                        }
                    } else {
                        if matched_color.is_none() {
                            matched_color = Some(step.color);
                        }
                    }
                }
            }
        }
        matched_color
    }
}

/// Application mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum AppMode {
    Normal,
    Search,
    Fullscreen,
    Inspect,
    FullscreenInspect,
}

/// Global application state.
#[derive(Debug)]
pub(crate) struct AppState {
    /// Prometheus client for making requests.
    pub(crate) prometheus: prom::PromClient,
    /// Current time range window.
    pub(crate) range: Duration,
    /// Query step resolution.
    pub(crate) step: Duration,
    /// How often to refresh data.
    pub(crate) refresh_every: Duration,
    /// List of panels.
    pub(crate) panels: Vec<PanelState>,
    /// Timestamp of the last successful refresh.
    pub(crate) last_refresh: Instant,
    /// Query end timestamp used by the currently rendered data.
    pub(crate) view_end_ts: i64,
    /// Vertical scroll offset.
    pub(crate) vertical_scroll: usize,
    /// Dashboard title.
    pub(crate) title: String,
    /// Whether to show the debug bar.
    pub(crate) debug_bar: bool,
    /// Template variables (key -> value).
    pub(crate) vars: HashMap<String, String>,
    /// Prometheus-backed template variables imported from Grafana.
    pub(crate) query_vars: Vec<TemplateQueryVar>,
    /// Count of panels skipped during import.
    pub(crate) skipped_panels: usize,
    /// Index of the currently selected panel.
    pub(crate) selected_panel: usize,
    /// UI Theme.
    pub(crate) theme: Theme,
    /// Time offset from "now" for panning backward in time (0 = live mode).
    pub(crate) time_offset: Duration,
    /// Current application mode.
    pub(crate) mode: AppMode,
    /// Search query string.
    pub(crate) search_query: String,
    /// Filtered panel indices based on search query.
    pub(crate) search_results: Vec<usize>,
    /// Cursor X position (timestamp) for inspection.
    pub(crate) cursor_x: Option<f64>,
    /// Global marker set for rendering thresholds
    pub(crate) threshold_marker: String,
    /// Global runtime toggle for automatic grid rendering.
    pub(crate) autogrid_enabled: bool,
    /// Color used for automatic grid lines and labels.
    pub(crate) autogrid_color: Color,
}

impl AppState {
    /// Creates a new application state.
    ///
    /// # Arguments
    ///
    /// * `prometheus` - The Prometheus client.
    /// * `range` - The initial time range window.
    /// * `step` - The query resolution step.
    /// * `refresh_every` - The data refresh interval.
    /// * `title` - The dashboard title.
    /// * `panels` - The list of panels to display.
    /// * `skipped_panels` - The count of panels that were skipped during import.
    /// * `theme` - The UI theme to use.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        prometheus: prom::PromClient,
        range: Duration,
        step: Duration,
        refresh_every: Duration,
        title: String,
        panels: Vec<PanelState>,
        skipped_panels: usize,
        theme: Theme,
        threshold_marker: String,
    ) -> Self {
        Self {
            prometheus,
            range,
            step,
            refresh_every,
            panels,
            last_refresh: Instant::now() - refresh_every,
            view_end_ts: chrono::Utc::now().timestamp(),
            vertical_scroll: 0,
            title,
            debug_bar: false,
            vars: HashMap::new(),
            query_vars: Vec::new(),
            skipped_panels,
            selected_panel: 0,
            theme,
            time_offset: Duration::from_secs(0),
            mode: AppMode::Normal,
            search_query: String::new(),
            search_results: Vec::new(),
            cursor_x: None,
            threshold_marker,
            autogrid_enabled: true,
            autogrid_color: Color::DarkGray,
        }
    }

    /// Zoom in: halve the time range.
    pub(crate) fn zoom_in(&mut self) {
        self.range /= 2;
        if self.range < Duration::from_secs(10) {
            self.range = Duration::from_secs(10);
        }
    }

    /// Zoom out: double the time range.
    pub(crate) fn zoom_out(&mut self) {
        self.range *= 2;
        self.range = self.range.min(Duration::from_secs(7 * 24 * 3600));
    }

    /// Pan left: shift the time window backward.
    pub(crate) fn pan_left(&mut self) {
        // Shift by 25% of the current range
        let shift = self.range / 4;
        self.time_offset = self.time_offset.saturating_add(shift);
    }

    /// Automatically scroll to ensure the selected panel is visible.
    pub(crate) fn scroll_to_selected_panel(&mut self) {
        if let Some(panel) = self.panels.get(self.selected_panel) {
            if let Some(grid) = panel.grid {
                let py = grid.y;
                let ph = grid.h;
                let scroll_y = self.vertical_scroll as i32;
                let visible_height = 20;

                if py < scroll_y {
                    self.vertical_scroll = py as usize;
                } else if py + ph > scroll_y + visible_height {
                    self.vertical_scroll = (py + ph - visible_height).max(0) as usize;
                }
            }
        }
    }

    /// Selects the previous panel, keeping the dashboard scrolled to it.
    pub(crate) fn select_previous_panel(&mut self) {
        if self.selected_panel > 0 {
            self.selected_panel -= 1;
            self.scroll_to_selected_panel();
        }
    }

    /// Selects the next panel, keeping the dashboard scrolled to it.
    pub(crate) fn select_next_panel(&mut self) {
        if self.selected_panel < self.panels.len().saturating_sub(1) {
            self.selected_panel += 1;
            self.scroll_to_selected_panel();
        }
    }

    /// Pan right: shift the time window forward (toward "now").
    pub(crate) fn pan_right(&mut self) {
        // Shift by 25% of the current range
        let shift = self.range / 4;
        if self.time_offset > shift {
            self.time_offset = self.time_offset.saturating_sub(shift);
        } else {
            self.time_offset = Duration::from_secs(0); // Back to live mode
        }
    }

    /// Reset to live mode (time_offset = 0).
    pub(crate) fn reset_to_live(&mut self) {
        self.time_offset = Duration::from_secs(0);
    }

    /// Check if currently in live mode.
    pub(crate) fn is_live(&self) -> bool {
        self.time_offset.as_secs() == 0
    }

    /// Returns the displayed time window bounds.
    pub(crate) fn time_bounds(&self) -> (f64, f64) {
        let end_ts = self.view_end_ts as f64;
        (end_ts - self.range.as_secs_f64(), end_ts)
    }

    /// Moves the inspection cursor to the center of the displayed time window.
    pub(crate) fn center_cursor(&mut self) {
        let (start_ts, end_ts) = self.time_bounds();
        self.cursor_x = Some((start_ts + end_ts) / 2.0);
    }

    /// Move cursor left/right by one step.
    pub(crate) fn move_cursor(&mut self, direction: i32) {
        let (start_ts, end_ts) = self.time_bounds();

        if let Some(current_x) = self.cursor_x {
            let step_secs = self.step.as_secs_f64();
            let new_x = current_x + (direction as f64 * step_secs);
            self.cursor_x = Some(new_x.max(start_ts).min(end_ts));
        } else {
            self.cursor_x = Some((start_ts + end_ts) / 2.0);
        }
    }

    pub(crate) async fn refresh(&mut self) -> Result<()> {
        let range = self.range;
        let step = self.step;

        // Calculate end timestamp: "now" minus time_offset
        let end_ts = chrono::Utc::now().timestamp() - self.time_offset.as_secs() as i64;

        let _ = refresh_query_variables(
            &self.prometheus,
            &self.query_vars,
            range,
            step,
            end_ts,
            &mut self.vars,
        )
        .await;

        let prometheus = &self.prometheus;
        let vars = &self.vars;

        // Create a stream of futures for fetching panel data
        let mut futures = futures::stream::iter(self.panels.iter_mut())
            .map(|p| Self::fetch_single_panel_data(prometheus, p, range, step, vars, end_ts))
            .buffer_unordered(4); // Max 4 concurrent panel refreshes

        while let Some((p, results, url, err)) = futures.next().await {
            p.series = results;
            p.last_samples = p.series.iter().map(|s| s.points.len()).sum();
            if let Some(u) = url {
                p.last_url = Some(u);
            }
            p.last_error = err;
        }

        self.view_end_ts = end_ts;
        self.last_refresh = Instant::now();
        Ok(())
    }

    async fn fetch_single_panel_data<'a>(
        prometheus: &'a prom::PromClient,
        p: &'a mut PanelState,
        range: Duration,
        step: Duration,
        vars: &'a HashMap<String, String>,
        end_ts: i64,
    ) -> (
        &'a mut PanelState,
        Vec<SeriesView>,
        Option<String>,
        Option<String>,
    ) {
        let mut panel_results = Vec::new();
        let mut last_url = None;
        let mut error = None;

        for (i, expr) in p.exprs.iter().enumerate() {
            let expr_expanded = expand_expr(expr, range, step, vars);
            let legend_fmt = p.legends.get(i).and_then(|x| x.as_ref());

            // Calculate start/end for URL display purposes
            let start_ts = end_ts - (range.as_secs() as i64);

            let url = prometheus.build_query_range_url(&expr_expanded, start_ts, end_ts, step);
            last_url = Some(url);

            match prometheus
                .query_range(&expr_expanded, start_ts, end_ts, step)
                .await
            {
                Ok(res) => {
                    for s in res {
                        let latest_val = s.values.last().and_then(|(_, v)| v.parse::<f64>().ok());
                        let legend_base = if let Some(fmt) = legend_fmt {
                            format_legend(fmt, &s.metric)
                        } else if s.metric.is_empty() {
                            expr_expanded.clone()
                        } else {
                            let mut labels: Vec<_> = s
                                .metric
                                .iter()
                                .map(|(k, v)| format!("{}=\"{}\"", k, v))
                                .collect();
                            labels.sort();
                            format!("{} {{{}}}", expr_expanded, labels.join(", "))
                        };

                        let mut pts = Vec::with_capacity(s.values.len());
                        for (ts, val) in s.values {
                            if let Ok(y) = val.parse::<f64>() {
                                if y.is_finite() {
                                    pts.push((ts, y));
                                }
                            }
                        }
                        panel_results.push(SeriesView {
                            name: legend_base,
                            value: latest_val,
                            points: downsample(pts, 200),
                            visible: true,
                        });
                    }
                }
                Err(e) => {
                    error = Some(format!("query_range failed for `{}`: {}", expr_expanded, e));
                }
            }
        }
        (p, panel_results, last_url, error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_app() -> AppState {
        AppState::new(
            prom::PromClient::new("http://localhost:9090".to_string()),
            Duration::from_secs(3600),
            Duration::from_secs(60),
            Duration::from_millis(1000),
            "Test".to_string(),
            vec![],
            0,
            Theme::default(),
            "dashed".to_string(),
        )
    }

    #[tokio::test]
    async fn test_empty_panels() {
        let mut app = create_test_app();

        assert!(app.refresh().await.is_ok());

        app.scroll_to_selected_panel();
        assert_eq!(app.selected_panel, 0);

        app.move_cursor(1);
    }

    #[test]
    fn test_time_bounds_use_refreshed_window() {
        let mut app = create_test_app();
        app.view_end_ts = 1_700_000_000;

        assert_eq!(app.time_bounds(), (1_699_996_400.0, 1_700_000_000.0));

        app.time_offset = Duration::from_secs(300);
        assert_eq!(app.time_bounds(), (1_699_996_400.0, 1_700_000_000.0));
    }

    #[test]
    fn test_center_and_move_cursor_use_refreshed_window() {
        let mut app = create_test_app();
        app.view_end_ts = 1_700_000_000;

        app.center_cursor();
        assert_eq!(app.cursor_x, Some(1_699_998_200.0));

        app.cursor_x = Some(1_700_000_000.0);
        app.move_cursor(1);
        assert_eq!(app.cursor_x, Some(1_700_000_000.0));

        app.cursor_x = Some(1_699_996_400.0);
        app.move_cursor(-1);
        assert_eq!(app.cursor_x, Some(1_699_996_400.0));
    }

    #[test]
    fn test_select_panel_navigation_is_bounded() {
        let prom = prom::PromClient::new("http://localhost:9090".to_string());
        let mut app = AppState::new(
            prom,
            Duration::from_secs(3600),
            Duration::from_secs(60),
            Duration::from_millis(1000),
            "Test".to_string(),
            crate::app::default_queries(vec![
                "up".to_string(),
                "process_cpu_seconds_total".to_string(),
            ]),
            0,
            Theme::default(),
            "dashed".to_string(),
        );

        app.select_previous_panel();
        assert_eq!(app.selected_panel, 0);

        app.select_next_panel();
        assert_eq!(app.selected_panel, 1);

        app.select_next_panel();
        assert_eq!(app.selected_panel, 1);

        app.select_previous_panel();
        assert_eq!(app.selected_panel, 0);
    }
}
