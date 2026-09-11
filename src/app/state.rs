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

use crate::app::data::{downsample, expand_expr, format_legend};
use crate::app::variables::refresh_query_variables;
use crate::dashboard::{DashboardItemId, DashboardLayout, RowId};
use crate::export::{ExportOptions, RecordingState};
use crate::grafana::TemplateQueryVar;
use crate::prom;
use crate::theme::Theme;
use crate::ui::DisplayFormat;
use anyhow::Result;
use futures::StreamExt;
use ratatui::style::Color;
use std::collections::{HashMap, HashSet};
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
    /// Query mode for each expression. Parallel to exprs.
    pub(crate) query_modes: Vec<QueryMode>,
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
    /// Display formatting imported from Grafana field configuration.
    pub(crate) display: DisplayFormat,
    /// Renderer-specific presentation options.
    pub(crate) options: PanelOptions,
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

/// Renderer-specific options carried by a generic panel.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) enum PanelOptions {
    #[default]
    None,
    Graph(GraphOptions),
}

/// Graph/timeseries rendering options imported from Grafana.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GraphOptions {
    pub(crate) draw_style: GraphDrawStyle,
    pub(crate) show_points: GraphPointMode,
    pub(crate) fill_opacity: Option<u8>,
    pub(crate) axis_placement: GraphAxisPlacement,
    pub(crate) line_interpolation: Option<String>,
    pub(crate) stacking: GraphStackingMode,
}

impl Default for GraphOptions {
    fn default() -> Self {
        Self {
            draw_style: GraphDrawStyle::Line,
            show_points: GraphPointMode::Auto,
            fill_opacity: None,
            axis_placement: GraphAxisPlacement::Visible,
            line_interpolation: None,
            stacking: GraphStackingMode::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphDrawStyle {
    Line,
    Points,
    Bars,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphPointMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphAxisPlacement {
    Visible,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphStackingMode {
    Off,
    Normal,
    Percent,
}

/// Prometheus endpoint mode for a target query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QueryMode {
    Range,
    Instant,
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
    pub(crate) fn graph_options(&self) -> GraphOptions {
        match &self.options {
            PanelOptions::Graph(options) => options.clone(),
            PanelOptions::None => GraphOptions::default(),
        }
    }

    pub(crate) fn query_mode(&self, index: usize) -> QueryMode {
        self.query_modes
            .get(index)
            .copied()
            .unwrap_or(QueryMode::Range)
    }

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
    /// Optional external point-event state.
    pub(crate) annotations: crate::annotations::AnnotationState,
    /// Owned active annotation cluster produced by the selected panel's latest draw.
    pub(crate) rendered_annotation_cluster: Option<Vec<crate::annotations::AnnotationEvent>>,
    /// Active annotation exploration modal, if any.
    pub(crate) annotation_modal: Option<crate::annotations::AnnotationModal>,
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
    /// Recursive row/panel dashboard layout.
    pub(crate) layout: DashboardLayout,
    /// Currently selected visible row or panel.
    pub(crate) selected_item: Option<DashboardItemId>,
    /// UI Theme.
    pub(crate) theme: Theme,
    /// Time offset from "now" for panning backward in time (0 = live mode).
    pub(crate) time_offset: Duration,
    /// Current application mode.
    pub(crate) mode: AppMode,
    /// Search query string.
    pub(crate) search_query: String,
    /// Filtered dashboard items based on search query.
    pub(crate) search_results: Vec<DashboardItemId>,
    /// Cursor X position (timestamp) for inspection.
    pub(crate) cursor_x: Option<f64>,
    /// Global marker set for rendering thresholds
    pub(crate) threshold_marker: String,
    /// Global runtime toggle for automatic grid rendering.
    pub(crate) autogrid_enabled: bool,
    /// Color used for automatic grid lines and labels.
    pub(crate) autogrid_color: Color,
    /// Image export and recording configuration.
    pub(crate) export: ExportOptions,
    /// Active frame recording state, if recording is enabled.
    pub(crate) recording: Option<RecordingState>,
    /// Last export or recording status message.
    pub(crate) export_status: Option<String>,
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
        export: ExportOptions,
    ) -> Self {
        let layout = DashboardLayout::flat(panels.len());
        let selected_item = layout.first_visible();
        Self {
            prometheus,
            annotations: crate::annotations::AnnotationState::from_path(None),
            rendered_annotation_cluster: None,
            annotation_modal: None,
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
            layout,
            selected_item,
            theme,
            time_offset: Duration::from_secs(0),
            mode: AppMode::Normal,
            search_query: String::new(),
            search_results: Vec::new(),
            cursor_x: None,
            threshold_marker,
            autogrid_enabled: true,
            autogrid_color: Color::DarkGray,
            export,
            recording: None,
            export_status: None,
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
        if let Some(panel) = self
            .selected_panel_index()
            .and_then(|index| self.panels.get(index))
            && let Some(grid) = panel.grid
        {
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

    pub(crate) fn apply_layout(&mut self, layout: DashboardLayout) {
        self.layout = layout;
        self.selected_item = self.layout.first_visible();
        self.scroll_to_selected_panel();
    }

    pub(crate) fn selected_panel_index(&self) -> Option<usize> {
        match self.selected_item {
            Some(DashboardItemId::Panel(index)) => Some(index),
            Some(DashboardItemId::Row(_)) | None => None,
        }
    }

    pub(crate) fn selected_row_id(&self) -> Option<RowId> {
        match self.selected_item {
            Some(DashboardItemId::Row(id)) => Some(id),
            Some(DashboardItemId::Panel(_)) | None => None,
        }
    }

    pub(crate) fn visible_panel_indices(&self) -> Vec<usize> {
        self.layout.visible_panel_indices()
    }

    /// Selects the previous visible dashboard item.
    pub(crate) fn select_previous_item(&mut self) {
        self.move_selection(-1);
    }

    /// Selects the next visible dashboard item.
    pub(crate) fn select_next_item(&mut self) {
        self.move_selection(1);
    }

    fn move_selection(&mut self, direction: isize) {
        let visible = self.layout.visible_items();
        let Some(first) = visible.first() else {
            self.selected_item = None;
            return;
        };
        let Some(selected) = self.selected_item else {
            self.selected_item = Some(first.id);
            self.scroll_to_selected_panel();
            return;
        };
        let Some(current) = visible.iter().position(|item| item.id == selected) else {
            self.selected_item = Some(first.id);
            self.scroll_to_selected_panel();
            return;
        };
        let next = current
            .saturating_add_signed(direction)
            .min(visible.len() - 1);
        self.selected_item = Some(visible[next].id);
        self.scroll_to_selected_panel();
    }

    fn ensure_selection_visible(&mut self) {
        self.selected_item = self
            .selected_item
            .and_then(|item| self.layout.nearest_visible_ancestor(item))
            .or_else(|| self.layout.first_visible());
        self.scroll_to_selected_panel();
    }

    pub(crate) async fn set_selected_row_collapsed(&mut self, collapsed: bool) -> Result<()> {
        let Some(row_id) = self.selected_row_id() else {
            return Ok(());
        };
        let Some(change) = self.layout.set_row_collapsed(row_id, collapsed) else {
            return Ok(());
        };
        if !change.newly_visible_panels.is_empty() {
            self.refresh_panel_indices(&change.newly_visible_panels, false)
                .await;
        }
        self.reconcile_visible_annotation_targets();
        self.ensure_selection_visible();
        Ok(())
    }

    pub(crate) async fn toggle_selected_row(&mut self) -> Result<()> {
        let Some(row_id) = self.selected_row_id() else {
            return Ok(());
        };
        let Some(change) = self.layout.toggle_row(row_id) else {
            return Ok(());
        };
        if !change.newly_visible_panels.is_empty() {
            self.refresh_panel_indices(&change.newly_visible_panels, false)
                .await;
        }
        self.reconcile_visible_annotation_targets();
        self.ensure_selection_visible();
        Ok(())
    }

    /// Compatibility wrapper for panel-only fullscreen navigation.
    pub(crate) fn select_previous_panel(&mut self) {
        self.move_panel_selection(-1);
    }

    /// Compatibility wrapper for panel-only fullscreen navigation.
    pub(crate) fn select_next_panel(&mut self) {
        self.move_panel_selection(1);
    }

    fn move_panel_selection(&mut self, direction: isize) {
        let panels = self.visible_panel_indices();
        let Some(&first) = panels.first() else {
            self.selected_item = None;
            return;
        };
        let Some(current) = self.selected_panel_index() else {
            self.selected_item = Some(DashboardItemId::Panel(first));
            self.scroll_to_selected_panel();
            return;
        };
        let Some(index) = panels.iter().position(|&panel| panel == current) else {
            self.selected_item = Some(DashboardItemId::Panel(first));
            self.scroll_to_selected_panel();
            return;
        };
        let next = index.saturating_add_signed(direction).min(panels.len() - 1);
        self.selected_item = Some(DashboardItemId::Panel(panels[next]));
        self.scroll_to_selected_panel();
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

    pub(crate) fn open_rendered_annotation_cluster(&mut self) {
        let Some(events) = self.rendered_annotation_cluster.clone() else {
            return;
        };
        if let Some(modal) = crate::annotations::ClusterModalState::new(events) {
            self.annotation_modal = Some(crate::annotations::AnnotationModal::Cluster(modal));
        }
    }

    pub(crate) fn open_tag_filter_modal(&mut self) {
        if let Some(modal) = self.annotations.new_tag_filter_modal() {
            self.annotation_modal = Some(crate::annotations::AnnotationModal::TagFilter(modal));
        }
    }

    pub(crate) async fn refresh(&mut self) -> Result<()> {
        let range = self.range;
        let step = self.step;
        let visible_panel_indices = self.visible_panel_indices();

        // Calculate end timestamp: "now" minus time_offset
        let end_ts = chrono::Utc::now().timestamp() - self.time_offset.as_secs() as i64;
        let annotation_context =
            crate::annotations::AnnotationRefreshContext::from_unix_window(end_ts, range);
        let annotation_refresh = self.annotations.refresh(&annotation_context);
        let prometheus_refresh = Self::refresh_prometheus_data(
            &self.prometheus,
            &self.query_vars,
            range,
            step,
            end_ts,
            &mut self.vars,
            &mut self.panels,
            &visible_panel_indices,
            true,
        );
        let (_, ()) = tokio::join!(annotation_refresh, prometheus_refresh);

        self.reconcile_visible_annotation_targets();

        self.view_end_ts = end_ts;
        self.last_refresh = Instant::now();
        Ok(())
    }

    fn reconcile_visible_annotation_targets(&mut self) {
        let visible_panels = self
            .visible_panel_indices()
            .into_iter()
            .collect::<HashSet<_>>();
        let titles = self
            .panels
            .iter()
            .enumerate()
            .filter(|(index, _)| visible_panels.contains(index))
            .filter(|(_, panel)| panel.panel_type == PanelType::Graph)
            .map(|(_, panel)| panel.title.clone())
            .collect::<Vec<_>>();
        self.annotations.reconcile_targets(&titles);
    }

    async fn refresh_panel_indices(&mut self, indices: &[usize], refresh_variables: bool) {
        let range = self.range;
        let step = self.step;
        let end_ts = self.view_end_ts;
        Self::refresh_prometheus_data(
            &self.prometheus,
            &self.query_vars,
            range,
            step,
            end_ts,
            &mut self.vars,
            &mut self.panels,
            indices,
            refresh_variables,
        )
        .await;
    }

    #[allow(clippy::too_many_arguments)]
    async fn refresh_prometheus_data(
        prometheus: &prom::PromClient,
        query_vars: &[TemplateQueryVar],
        range: Duration,
        step: Duration,
        end_ts: i64,
        vars: &mut HashMap<String, String>,
        panels: &mut [PanelState],
        indices: &[usize],
        refresh_variables: bool,
    ) {
        if refresh_variables {
            let _ =
                refresh_query_variables(prometheus, query_vars, range, step, end_ts, vars).await;
        }

        // Create a stream of futures for fetching panel data
        let indices = indices.iter().copied().collect::<HashSet<_>>();
        let mut futures = futures::stream::iter(
            panels
                .iter_mut()
                .enumerate()
                .filter_map(|(index, panel)| indices.contains(&index).then_some(panel)),
        )
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
            let query_mode = p.query_mode(i);

            // Calculate start/end for URL display purposes
            let start_ts = end_ts - (range.as_secs() as i64);

            let url = match query_mode {
                QueryMode::Range => {
                    prometheus.build_query_range_url(&expr_expanded, start_ts, end_ts, step)
                }
                QueryMode::Instant => prometheus.build_query_url(&expr_expanded, end_ts),
            };
            last_url = Some(url);

            let query_result = match query_mode {
                QueryMode::Range => {
                    prometheus
                        .query_range(&expr_expanded, start_ts, end_ts, step)
                        .await
                }
                QueryMode::Instant => {
                    prometheus
                        .query_instant_series(&expr_expanded, end_ts)
                        .await
                }
            };

            match query_result {
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
                            if let Ok(y) = val.parse::<f64>()
                                && y.is_finite()
                            {
                                pts.push((ts, y));
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
                    let query_name = match query_mode {
                        QueryMode::Range => "query_range",
                        QueryMode::Instant => "query",
                    };
                    error = Some(format!(
                        "{} failed for `{}`: {}",
                        query_name, expr_expanded, e
                    ));
                }
            }
        }
        (p, panel_results, last_url, error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotations::{
        AnnotationProvider, AnnotationRefreshContext, AnnotationSnapshot, ProviderFuture,
        ProviderPoll,
    };
    use crate::dashboard::{
        DashboardItemId, DashboardLayout, DashboardLayoutItem, DashboardRow, RowId,
    };
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::Notify;

    fn temp_annotation_path(name: &str) -> std::path::PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "grafatui-app-{name}-{}-{suffix}.jsonl",
            std::process::id()
        ))
    }

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
            ExportOptions::default(),
        )
    }

    fn test_panel(title: &str, panel_type: PanelType) -> PanelState {
        PanelState {
            title: title.to_string(),
            exprs: vec![],
            legends: vec![],
            query_modes: vec![],
            series: vec![],
            last_error: None,
            last_url: None,
            last_samples: 0,
            grid: None,
            y_axis_mode: YAxisMode::Auto,
            panel_type,
            thresholds: None,
            min: None,
            max: None,
            autogrid: None,
            display: crate::ui::DisplayFormat::default(),
            options: PanelOptions::None,
        }
    }

    fn row_test_layout(parent_collapsed: bool) -> DashboardLayout {
        DashboardLayout::new(vec![DashboardLayoutItem::Row(DashboardRow::new(
            RowId::new(0),
            "Parent",
            parent_collapsed,
            false,
            vec![
                DashboardLayoutItem::Panel(0),
                DashboardLayoutItem::Row(DashboardRow::new(
                    RowId::new(1),
                    "Nested",
                    true,
                    false,
                    vec![DashboardLayoutItem::Panel(1)],
                )),
            ],
        ))])
    }

    fn row_test_app() -> AppState {
        let mut app = create_test_app();
        app.panels = vec![
            test_panel("Visible", PanelType::Graph),
            test_panel("Nested hidden", PanelType::Graph),
        ];
        app.apply_layout(row_test_layout(false));
        app
    }

    fn row_test_app_with_queries(base_url: &str) -> AppState {
        let mut app = create_test_app();
        app.prometheus = prom::PromClient::new(base_url.to_string());
        app.panels = vec![
            test_panel("Visible", PanelType::Graph),
            test_panel("Nested hidden", PanelType::Graph),
        ];
        for panel in &mut app.panels {
            panel.exprs = vec!["up".to_string()];
            panel.legends = vec![None];
            panel.query_modes = vec![QueryMode::Range];
        }
        app.apply_layout(row_test_layout(true));
        app
    }

    #[tokio::test]
    async fn navigation_uses_visible_items_and_preserves_nested_state() {
        let mut app = row_test_app();
        assert_eq!(app.selected_item, Some(DashboardItemId::Row(RowId::new(0))));

        app.select_next_item();
        assert_eq!(app.selected_panel_index(), Some(0));
        app.select_previous_item();
        app.toggle_selected_row().await.unwrap();

        assert!(app.visible_panel_indices().is_empty());
        assert_eq!(app.selected_item, Some(DashboardItemId::Row(RowId::new(0))));

        app.toggle_selected_row().await.unwrap();
        assert!(app.layout.row(RowId::new(1)).unwrap().collapsed);
        assert_eq!(app.visible_panel_indices(), vec![0]);
    }

    #[test]
    fn fullscreen_navigation_skips_visible_row_items() {
        let mut app = create_test_app();
        app.panels = vec![
            test_panel("First", PanelType::Graph),
            test_panel("Second", PanelType::Graph),
        ];
        app.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::Row(
            DashboardRow::new(
                RowId::new(0),
                "Parent",
                false,
                false,
                vec![
                    DashboardLayoutItem::Panel(0),
                    DashboardLayoutItem::Row(DashboardRow::new(
                        RowId::new(1),
                        "Nested",
                        false,
                        false,
                        vec![DashboardLayoutItem::Panel(1)],
                    )),
                ],
            ),
        )]));
        app.selected_item = Some(DashboardItemId::Panel(0));

        app.select_next_panel();

        assert_eq!(app.selected_item, Some(DashboardItemId::Panel(1)));
        app.select_previous_panel();
        assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
    }

    #[test]
    fn empty_dashboard_has_no_selection_and_navigation_is_safe() {
        let mut app = create_test_app();

        assert_eq!(app.selected_item, None);
        assert_eq!(app.selected_panel_index(), None);
        app.select_previous_item();
        app.select_next_item();

        assert_eq!(app.selected_item, None);
    }

    #[tokio::test]
    async fn refresh_skips_collapsed_panels_and_expand_fetches_only_newly_visible() {
        let mut app = row_test_app_with_queries("http://127.0.0.1:9");

        app.refresh().await.unwrap();
        assert!(app.panels[0].last_error.is_none());
        assert!(app.panels[1].last_error.is_none());

        app.toggle_selected_row().await.unwrap();
        assert!(app.panels[0].last_error.is_some());
        assert!(app.panels[1].last_error.is_none());
    }

    #[tokio::test]
    async fn expansion_uses_the_displayed_window_without_resetting_refresh_clock() {
        let mut app = row_test_app_with_queries("http://127.0.0.1:9");
        app.view_end_ts = 1_700_000_000;
        app.last_refresh = Instant::now() - Duration::from_secs(30);
        let last_refresh = app.last_refresh;

        app.toggle_selected_row().await.unwrap();

        assert_eq!(app.view_end_ts, 1_700_000_000);
        assert_eq!(app.last_refresh, last_refresh);
        assert!(
            app.panels[0]
                .last_url
                .as_deref()
                .unwrap()
                .contains("end=1700000000")
        );
    }

    #[tokio::test]
    async fn refresh_reconciles_annotations_against_visible_graph_panels_only() {
        let path = temp_annotation_path("visible-target-reconciliation");
        let time = chrono::Utc::now().to_rfc3339();
        std::fs::write(
            &path,
            format!("{{\"time\":\"{time}\",\"text\":\"cpu\",\"panel_titles\":[\"CPU\"]}}\n"),
        )
        .unwrap();
        let mut app = create_test_app();
        app.panels = vec![
            test_panel("CPU", PanelType::Graph),
            test_panel("CPU", PanelType::Graph),
        ];
        app.apply_layout(DashboardLayout::new(vec![
            DashboardLayoutItem::Panel(0),
            DashboardLayoutItem::Row(DashboardRow::new(
                RowId::new(0),
                "Hidden duplicate",
                true,
                false,
                vec![DashboardLayoutItem::Panel(1)],
            )),
        ]));
        app.annotations = crate::annotations::AnnotationState::from_path(Some(path.clone()));

        app.refresh().await.unwrap();

        assert_eq!(app.annotations.footer_status(), None);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn collapsing_a_row_reconciles_unchanged_annotation_targets() {
        let mut event = crate::annotations::test_event_at(50.0, "cpu");
        event.target = crate::annotations::AnnotationTarget::PanelTitles(
            ["CPU".to_string()].into_iter().collect(),
        );
        let mut app = create_test_app();
        app.panels = vec![
            test_panel("CPU", PanelType::Graph),
            test_panel("CPU", PanelType::Graph),
        ];
        app.apply_layout(DashboardLayout::new(vec![
            DashboardLayoutItem::Panel(0),
            DashboardLayoutItem::Row(DashboardRow::new(
                RowId::new(0),
                "Collapsible",
                false,
                false,
                vec![DashboardLayoutItem::Panel(1)],
            )),
        ]));
        app.annotations = crate::annotations::AnnotationState::from_events_for_test(vec![event]);
        app.annotations
            .reconcile_targets(&["CPU".to_string(), "CPU".to_string()]);
        assert!(app.annotations.footer_status().is_some());
        app.selected_item = Some(DashboardItemId::Row(RowId::new(0)));

        app.set_selected_row_collapsed(true).await.unwrap();

        assert_eq!(app.annotations.footer_status(), None);
    }

    #[derive(Debug)]
    struct RecordingProvider {
        captured: Arc<Mutex<Option<AnnotationRefreshContext>>>,
    }

    impl AnnotationProvider for RecordingProvider {
        fn refresh<'a>(&'a mut self, context: &'a AnnotationRefreshContext) -> ProviderFuture<'a> {
            let captured = Arc::clone(&self.captured);
            let context = context.clone();
            Box::pin(async move {
                *captured.lock().unwrap() = Some(context);
                ProviderPoll::Loaded(AnnotationSnapshot::new(Vec::new()))
            })
        }
    }

    #[derive(Debug)]
    struct HandshakeProvider {
        provider_started: Arc<Notify>,
        metrics_started: Arc<Notify>,
    }

    impl AnnotationProvider for HandshakeProvider {
        fn refresh<'a>(&'a mut self, _context: &'a AnnotationRefreshContext) -> ProviderFuture<'a> {
            let provider_started = Arc::clone(&self.provider_started);
            let metrics_started = Arc::clone(&self.metrics_started);
            Box::pin(async move {
                provider_started.notify_one();
                metrics_started.notified().await;
                ProviderPoll::Loaded(AnnotationSnapshot::new(Vec::new()))
            })
        }
    }

    #[tokio::test]
    async fn test_empty_panels() {
        let mut app = create_test_app();

        assert!(app.refresh().await.is_ok());

        app.scroll_to_selected_panel();
        assert_eq!(app.selected_panel_index(), None);

        app.move_cursor(1);
    }

    #[tokio::test]
    async fn refresh_reloads_annotations_without_panels() {
        let path = temp_annotation_path("app-refresh");
        std::fs::write(
            &path,
            "{\"time\":\"2026-07-23T14:30:00Z\",\"text\":\"deploy\"}\n",
        )
        .unwrap();
        let mut app = create_test_app();
        app.annotations = crate::annotations::AnnotationState::from_path(Some(path.clone()));

        app.refresh().await.unwrap();

        assert_eq!(app.annotations.snapshot().unwrap().len(), 1);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn refresh_uses_same_window_for_annotations_and_rendered_data() {
        let captured = Arc::new(Mutex::new(None));
        let mut app = create_test_app();
        app.annotations =
            crate::annotations::AnnotationState::from_provider(Some(Box::new(RecordingProvider {
                captured: Arc::clone(&captured),
            })));

        app.refresh().await.unwrap();

        let captured = captured.lock().unwrap().clone().unwrap();
        assert_eq!(captured.to.timestamp(), app.view_end_ts);
        assert_eq!(captured.from, captured.to - chrono::TimeDelta::hours(1));
    }

    #[tokio::test]
    async fn refresh_starts_annotation_and_prometheus_work_concurrently() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let provider_started = Arc::new(Notify::new());
        let metrics_started = Arc::new(Notify::new());
        let http_provider_started = Arc::clone(&provider_started);
        let http_metrics_started = Arc::clone(&metrics_started);
        let http_task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = socket.read(&mut chunk).await.unwrap();
                assert!(read > 0, "connection closed before request headers");
                request.extend_from_slice(&chunk[..read]);
            }
            http_metrics_started.notify_one();
            http_provider_started.notified().await;

            let body = r#"{"status":"success","data":{"resultType":"matrix","result":[]}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body,
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let mut app = create_test_app();
        app.prometheus = prom::PromClient::new(format!("http://{address}"));
        let mut panel = test_panel("Up", PanelType::Graph);
        panel.exprs = vec!["up".to_string()];
        panel.legends = vec![None];
        panel.query_modes = vec![QueryMode::Range];
        app.panels = vec![panel];
        app.apply_layout(DashboardLayout::flat(app.panels.len()));
        app.annotations =
            crate::annotations::AnnotationState::from_provider(Some(Box::new(HandshakeProvider {
                provider_started,
                metrics_started,
            })));

        tokio::time::timeout(Duration::from_secs(2), app.refresh())
            .await
            .expect("annotation and Prometheus refresh did not start concurrently")
            .unwrap();
        http_task.await.unwrap();
        assert!(app.panels[0].last_error.is_none());
    }

    #[tokio::test]
    async fn reload_preserves_open_cluster_modal() {
        let path = temp_annotation_path("open-cluster-reload");
        std::fs::write(
            &path,
            "{\"time\":\"2026-07-23T14:30:00Z\",\"text\":\"initial\"}\n",
        )
        .unwrap();
        let mut app = create_test_app();
        app.annotations = crate::annotations::AnnotationState::from_path(Some(path.clone()));
        app.refresh().await.unwrap();
        app.rendered_annotation_cluster =
            Some(app.annotations.snapshot().unwrap().events().to_vec());
        app.open_rendered_annotation_cluster();

        std::fs::write(
            &path,
            "{\"time\":\"2026-07-23T14:31:00Z\",\"text\":\"replacement event\"}\n",
        )
        .unwrap();
        app.refresh().await.unwrap();

        assert_eq!(
            app.annotations.snapshot().unwrap().events()[0].text,
            "replacement event"
        );
        let Some(crate::annotations::AnnotationModal::Cluster(modal)) =
            app.annotation_modal.as_ref()
        else {
            panic!("cluster modal should remain open");
        };
        assert_eq!(modal.events()[0].text, "initial");
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn refresh_reconciles_annotation_targets() {
        let path = temp_annotation_path("target-reconciliation");
        std::fs::write(
            &path,
            concat!(
                r#"{"time":"2026-08-11T14:30:00Z","text":"cpu","panel_titles":["CPU"]}"#,
                "\n",
                r#"{"time":"2026-08-11T14:31:00Z","text":"stat","panel_titles":["OnlyStat"]}"#,
                "\n",
                r#"{"time":"2026-08-11T14:32:00Z","text":"missing","panel_titles":["Missing"]}"#,
                "\n",
            ),
        )
        .unwrap();
        let mut app = create_test_app();
        app.panels = vec![
            test_panel("CPU", PanelType::Graph),
            test_panel("CPU", PanelType::Graph),
            test_panel("OnlyStat", PanelType::Stat),
        ];
        app.apply_layout(DashboardLayout::flat(app.panels.len()));
        app.annotations = crate::annotations::AnnotationState::from_path(Some(path.clone()));

        app.refresh().await.unwrap();

        assert_eq!(
            app.annotations.footer_status(),
            Some(
                "target \"CPU\" matches 2 graph/timeseries panels; applied to all (+2 more)"
                    .to_string()
            )
        );
        std::fs::remove_file(path).unwrap();
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
            ExportOptions::default(),
        );

        app.select_previous_panel();
        assert_eq!(app.selected_panel_index(), Some(0));

        app.select_next_panel();
        assert_eq!(app.selected_panel_index(), Some(1));

        app.select_next_panel();
        assert_eq!(app.selected_panel_index(), Some(1));

        app.select_previous_panel();
        assert_eq!(app.selected_panel_index(), Some(0));
    }

    #[test]
    fn test_panel_query_mode_defaults_to_range_when_missing() {
        let panel = PanelState {
            title: "Modes".to_string(),
            exprs: vec!["up".to_string(), "rate(up[5m])".to_string()],
            legends: vec![None, None],
            query_modes: vec![QueryMode::Instant],
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
            options: PanelOptions::None,
        };

        assert_eq!(panel.query_mode(0), QueryMode::Instant);
        assert_eq!(panel.query_mode(1), QueryMode::Range);
    }

    #[test]
    fn test_default_graph_options_match_current_line_rendering() {
        let options = GraphOptions::default();

        assert_eq!(options.draw_style, GraphDrawStyle::Line);
        assert_eq!(options.show_points, GraphPointMode::Auto);
        assert_eq!(options.fill_opacity, None);
        assert_eq!(options.axis_placement, GraphAxisPlacement::Visible);
        assert_eq!(options.line_interpolation, None);
        assert_eq!(options.stacking, GraphStackingMode::Off);
    }

    #[test]
    fn test_panel_graph_options_fall_back_to_defaults() {
        let panel = PanelState {
            title: "not graph".to_string(),
            exprs: vec![],
            legends: vec![],
            query_modes: vec![],
            series: vec![],
            last_error: None,
            last_url: None,
            last_samples: 0,
            grid: None,
            y_axis_mode: YAxisMode::Auto,
            panel_type: PanelType::Stat,
            thresholds: None,
            min: None,
            max: None,
            autogrid: None,
            display: crate::ui::DisplayFormat::default(),
            options: PanelOptions::None,
        };

        assert_eq!(panel.graph_options(), GraphOptions::default());
    }
}
