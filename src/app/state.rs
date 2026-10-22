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

use crate::app::data::{
    QueryIntervals, StepPolicy, downsample, expand_expr, format_legend, parse_min_interval,
};
use crate::app::variables::{
    VariableRefreshContext, VariableReport, VariableUpdate, refresh_query_variables,
};
use crate::dashboard::{DashboardItemId, DashboardLayout, RowId, TabGroupId};
use crate::export::{ExportOptions, RecordingState};
use crate::grafana::TemplateQueryVar;
use crate::prom;
use crate::theme::Theme;
use crate::ui::DisplayFormat;
use anyhow::Result;
use futures::StreamExt;
use ratatui::style::Color;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;

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
    /// Freshness and Prometheus diagnostics for the displayed data.
    pub(crate) notices: DataNotices,
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
    /// Grafana query options that set the range query step.
    pub(crate) resolution: QueryResolution,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DataNotices {
    /// Queries whose previous series remain because their latest fetch failed.
    pub(crate) stale_queries: usize,
    /// Warnings and infos returned with the latest successful query results.
    pub(crate) warnings: Vec<String>,
}

/// Grafana query options that set how finely a panel's range queries sample.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct QueryResolution {
    /// Panel min interval (Grafana `interval`), which may reference a variable.
    pub(crate) min_interval: Option<String>,
    /// Panel `maxDataPoints`.
    pub(crate) max_data_points: Option<u32>,
    /// Per-target min step (`targets[].interval`). Parallel to exprs.
    pub(crate) target_min_intervals: Vec<Option<String>>,
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
    Summary(SummaryCalc),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SummaryCalc {
    Last,
    LastNotNull,
    Min,
    Max,
    Mean,
    Sum,
}

fn reduce_values(values: &[(f64, String)], calc: SummaryCalc) -> Option<f64> {
    let finite = |value: &str| {
        value
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
    };
    match calc {
        SummaryCalc::Last => values.last().and_then(|(_, value)| finite(value)),
        SummaryCalc::LastNotNull => values.iter().rev().find_map(|(_, value)| finite(value)),
        SummaryCalc::Min => values
            .iter()
            .filter_map(|(_, value)| finite(value))
            .reduce(f64::min),
        SummaryCalc::Max => values
            .iter()
            .filter_map(|(_, value)| finite(value))
            .reduce(f64::max),
        SummaryCalc::Mean | SummaryCalc::Sum => {
            let (sum, count) = values
                .iter()
                .filter_map(|(_, value)| finite(value))
                .fold((0.0, 0usize), |(sum, count), value| {
                    (sum + value, count + 1)
                });
            if count == 0 {
                None
            } else {
                let result = if calc == SummaryCalc::Mean {
                    sum / count as f64
                } else {
                    sum
                };
                result.is_finite().then_some(result)
            }
        }
    }
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
    /// Index of the panel query that produced this series.
    pub(crate) query_index: usize,
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
            PanelOptions::None | PanelOptions::Summary(_) => GraphOptions::default(),
        }
    }

    pub(crate) fn query_mode(&self, index: usize) -> QueryMode {
        self.query_modes
            .get(index)
            .copied()
            .unwrap_or(QueryMode::Range)
    }

    /// Step and rate interval for the expression at `index`. A target's min
    /// interval takes precedence over the panel's, and either replaces both the
    /// dashboard minimum step and the scrape interval, as in Grafana.
    pub(crate) fn query_intervals(
        &self,
        index: usize,
        range: Duration,
        step_policy: StepPolicy,
        scrape_interval: Duration,
        vars: &HashMap<String, String>,
    ) -> QueryIntervals {
        let parse = |text: &str| {
            let text = super::variables::substitute_variables(text, |name| {
                vars.get(name).map(|value| Cow::Borrowed(value.as_str()))
            });
            parse_min_interval(&text)
        };
        let resolution = &self.resolution;
        let min_interval = resolution
            .target_min_intervals
            .get(index)
            .and_then(Option::as_deref)
            .and_then(parse)
            .or_else(|| resolution.min_interval.as_deref().and_then(parse));
        QueryIntervals::for_policy(
            range,
            step_policy,
            scrape_interval,
            min_interval,
            resolution.max_data_points,
        )
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
    /// Exact configured step, or automatic range-aware selection.
    pub(crate) step_policy: StepPolicy,
    /// Prometheus scrape interval, used for `$__rate_interval`.
    pub(crate) scrape_interval: Duration,
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
    /// Scroll state belongs to panel identity, not its visible position.
    pub(crate) panel_body_scroll: HashMap<usize, super::panel_scroll::PanelBodyScroll>,
    /// Dashboard title.
    pub(crate) title: String,
    /// Whether to show the debug bar.
    pub(crate) debug_bar: bool,
    /// Template variables (key -> value).
    pub(crate) vars: HashMap<String, String>,
    /// Prometheus-backed template variables imported from Grafana.
    pub(crate) query_vars: Vec<TemplateQueryVar>,
    pub(crate) variable_errors: BTreeMap<String, String>,
    pub(crate) variable_state: crate::dashboard::variables::VariableState,
    variables_loaded: bool,
    pub(crate) dynamic: Option<super::dynamic::DynamicDashboard>,
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
    /// * `step_policy` - Exact configured step or automatic step selection.
    /// * `refresh_every` - The data refresh interval.
    /// * `title` - The dashboard title.
    /// * `panels` - The list of panels to display.
    /// * `skipped_panels` - The count of panels that were skipped during import.
    /// * `theme` - The UI theme to use.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        prometheus: prom::PromClient,
        range: Duration,
        step_policy: impl Into<StepPolicy>,
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
            step_policy: step_policy.into(),
            scrape_interval: crate::app::data::DEFAULT_SCRAPE_INTERVAL,
            refresh_every,
            panels,
            last_refresh: Instant::now() - refresh_every,
            view_end_ts: chrono::Utc::now().timestamp(),
            vertical_scroll: 0,
            panel_body_scroll: HashMap::new(),
            title,
            debug_bar: false,
            vars: HashMap::new(),
            query_vars: Vec::new(),
            variable_errors: BTreeMap::new(),
            variable_state: Default::default(),
            variables_loaded: false,
            dynamic: None,
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
            Some(DashboardItemId::Row(_) | DashboardItemId::Tabs(_)) | None => None,
        }
    }

    pub(crate) fn selected_row_id(&self) -> Option<RowId> {
        match self.selected_item {
            Some(DashboardItemId::Row(id)) => Some(id),
            Some(DashboardItemId::Panel(_) | DashboardItemId::Tabs(_)) | None => None,
        }
    }

    pub(crate) fn selected_tab_group_id(&self) -> Option<TabGroupId> {
        match self.selected_item {
            Some(DashboardItemId::Tabs(id)) => Some(id),
            Some(DashboardItemId::Panel(_) | DashboardItemId::Row(_)) | None => None,
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

    pub(crate) async fn set_selected_row_collapsed(
        &mut self,
        collapsed: bool,
    ) -> Result<Option<Vec<usize>>> {
        let Some(row_id) = self.selected_row_id() else {
            return Ok(None);
        };
        let Some(change) = self.layout.set_row_collapsed(row_id, collapsed) else {
            return Ok(None);
        };
        self.reconcile_visible_annotation_targets();
        self.ensure_selection_visible();
        Ok(Some(change.newly_visible_panels))
    }

    pub(crate) async fn toggle_selected_row(&mut self) -> Result<Option<Vec<usize>>> {
        let Some(row_id) = self.selected_row_id() else {
            return Ok(None);
        };
        let Some(change) = self.layout.toggle_row(row_id) else {
            return Ok(None);
        };
        self.reconcile_visible_annotation_targets();
        self.ensure_selection_visible();
        Ok(Some(change.newly_visible_panels))
    }

    pub(crate) async fn activate_tab(
        &mut self,
        id: TabGroupId,
        index: usize,
    ) -> Result<Option<Vec<usize>>> {
        let Some(change) = self.layout.set_active_tab(id, index) else {
            return Ok(None);
        };
        self.selected_item = Some(DashboardItemId::Tabs(id));
        self.reconcile_visible_annotation_targets();
        self.ensure_selection_visible();
        Ok(Some(change.newly_visible_panels))
    }

    pub(crate) async fn move_selected_tab(
        &mut self,
        direction: isize,
    ) -> Result<Option<Vec<usize>>> {
        let Some(id) = self.selected_tab_group_id() else {
            return Ok(None);
        };
        let Some(group) = self.layout.tabs(id) else {
            return Ok(None);
        };
        let Some(active) = group.active else {
            return Ok(None);
        };
        let next = active
            .saturating_add_signed(direction)
            .min(group.tabs.len().saturating_sub(1));
        self.activate_tab(id, next).await
    }

    pub(crate) fn enter_selected_tab(&mut self) {
        let Some(id) = self.selected_tab_group_id() else {
            return;
        };
        if let Some(item) = self.layout.first_tab_descendant(id) {
            self.selected_item = Some(item);
        }
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
            let step_secs = self.default_intervals().step.as_secs_f64();
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

    #[cfg(test)]
    pub(crate) async fn refresh_initial(&mut self) -> Result<()> {
        self.refresh_with(VariableUpdate::Load).await?;
        // Startup has no user selection to preserve; resolve options/conditions first.
        self.selected_item = self.layout.first_visible();
        Ok(())
    }

    /// Intervals for queries without panel query options, such as variables.
    pub(crate) fn default_intervals(&self) -> QueryIntervals {
        QueryIntervals::for_policy(
            self.range,
            self.step_policy,
            self.scrape_interval,
            None,
            None,
        )
    }

    #[cfg(test)]
    pub(crate) async fn refresh(&mut self) -> Result<()> {
        self.refresh_with(VariableUpdate::TimeRangeChange).await
    }

    #[cfg(test)]
    pub(crate) async fn refresh_periodic(&mut self) -> Result<()> {
        self.refresh_with(VariableUpdate::Tick).await
    }

    #[cfg(test)]
    async fn refresh_with(&mut self, requested_update: VariableUpdate) -> Result<()> {
        let end_ts = chrono::Utc::now().timestamp() - self.time_offset.as_secs() as i64;
        self.refresh_with_at(requested_update, end_ts).await
    }

    async fn refresh_at(&mut self, update: VariableUpdate, end_ts: i64) -> Result<()> {
        let initial = !self.variables_loaded;
        self.refresh_with_at(update, end_ts).await?;
        if initial {
            self.selected_item = self.layout.first_visible();
        }
        Ok(())
    }

    async fn refresh_with_at(
        &mut self,
        requested_update: VariableUpdate,
        end_ts: i64,
    ) -> Result<()> {
        let variable_update = if self.variables_loaded {
            requested_update
        } else {
            VariableUpdate::Load
        };
        let range = self.range;
        let step_policy = self.step_policy;
        let scrape_interval = self.scrape_interval;
        let default_intervals = self.default_intervals();

        let annotation_context =
            crate::annotations::AnnotationRefreshContext::from_unix_window(end_ts, range);
        let mut annotations = std::mem::replace(
            &mut self.annotations,
            crate::annotations::AnnotationState::from_path(None),
        );
        let annotation_refresh = annotations.refresh(&annotation_context);
        let prometheus_refresh = async {
            if self.variable_state.has_variables() {
                let active_scopes = self.active_variable_scopes();
                crate::app::variables::refresh_scoped_variables(
                    &self.prometheus,
                    &mut self.variable_state,
                    &active_scopes,
                    VariableRefreshContext {
                        update: variable_update,
                        range,
                        intervals: default_intervals,
                        end_ts,
                    },
                    &self.vars,
                )
                .await;
            }
            self.reconcile_dynamic();
            let visible_panel_indices = self.query_eligible_indices();
            Self::refresh_prometheus_data(
                &self.prometheus,
                &self.query_vars,
                &self.variable_state,
                self.dynamic.as_mut(),
                range,
                step_policy,
                scrape_interval,
                end_ts,
                &mut self.vars,
                &mut self.panels,
                &visible_panel_indices,
                &self.variable_errors,
                Some(variable_update),
            )
            .await
        };
        let (_, variable_report) = tokio::join!(annotation_refresh, prometheus_refresh);
        self.annotations = annotations;
        self.apply_variable_report(variable_report);
        self.evaluate_dynamic_visibility();

        self.reconcile_visible_annotation_targets();

        self.view_end_ts = end_ts;
        self.variables_loaded = true;
        self.last_refresh = Instant::now();
        Ok(())
    }

    fn refresh_snapshot(&self) -> Self {
        Self {
            prometheus: self.prometheus.clone(),
            annotations: crate::annotations::AnnotationState::from_path(None),
            rendered_annotation_cluster: self.rendered_annotation_cluster.clone(),
            annotation_modal: self.annotation_modal.clone(),
            range: self.range,
            step_policy: self.step_policy,
            scrape_interval: self.scrape_interval,
            refresh_every: self.refresh_every,
            panels: self.panels.clone(),
            last_refresh: self.last_refresh,
            view_end_ts: self.view_end_ts,
            vertical_scroll: self.vertical_scroll,
            panel_body_scroll: self.panel_body_scroll.clone(),
            title: self.title.clone(),
            debug_bar: self.debug_bar,
            vars: self.vars.clone(),
            query_vars: self.query_vars.clone(),
            variable_errors: self.variable_errors.clone(),
            variable_state: self.variable_state.clone(),
            variables_loaded: self.variables_loaded,
            dynamic: self.dynamic.clone(),
            skipped_panels: self.skipped_panels,
            layout: self.layout.clone(),
            selected_item: self.selected_item,
            theme: self.theme.clone(),
            time_offset: self.time_offset,
            mode: self.mode,
            search_query: self.search_query.clone(),
            search_results: self.search_results.clone(),
            cursor_x: self.cursor_x,
            threshold_marker: self.threshold_marker.clone(),
            autogrid_enabled: self.autogrid_enabled,
            autogrid_color: self.autogrid_color,
            export: self.export.clone(),
            recording: None,
            export_status: self.export_status.clone(),
        }
    }

    /// Merge query-owned state without restoring the interaction state captured
    /// before the request. Dynamic panels are matched by stable instance identity.
    fn apply_refresh_snapshot(&mut self, mut snapshot: Self) {
        let remap = (0..self.panels.len())
            .filter_map(|index| {
                let next = match (&self.dynamic, &snapshot.dynamic) {
                    (Some(current), Some(next)) => current.remap_index(next, index),
                    (None, None) => (index < snapshot.panels.len()).then_some(index),
                    _ => None,
                }?;
                Some((index, next))
            })
            .collect::<HashMap<_, _>>();
        let map_item = |item| match item {
            DashboardItemId::Panel(index) => remap.get(&index).copied().map(DashboardItemId::Panel),
            other => Some(other),
        };
        let selected = self
            .selected_item
            .and_then(map_item)
            .or(snapshot.selected_item);
        for (&old, &new) in &remap {
            let current = &self.panels[old];
            let next = &mut snapshot.panels[new];
            next.y_axis_mode = current.y_axis_mode;
            for series in &mut next.series {
                if let Some(previous) = current.series.iter().find(|previous| {
                    previous.query_index == series.query_index && previous.name == series.name
                }) {
                    series.visible = previous.visible;
                }
            }
        }
        self.panel_body_scroll = std::mem::take(&mut self.panel_body_scroll)
            .into_iter()
            .filter_map(|(index, state)| remap.get(&index).map(|&next| (next, state)))
            .collect();
        self.search_results = std::mem::take(&mut self.search_results)
            .into_iter()
            .filter_map(map_item)
            .collect();
        self.panels = snapshot.panels;
        self.layout = snapshot.layout;
        self.dynamic = snapshot.dynamic;
        self.vars = snapshot.vars;
        self.variable_state = snapshot.variable_state;
        self.variable_errors = snapshot.variable_errors;
        self.variables_loaded = snapshot.variables_loaded;
        self.view_end_ts = snapshot.view_end_ts;
        self.last_refresh = snapshot.last_refresh;
        self.selected_item = selected
            .and_then(|item| self.layout.nearest_visible_ancestor(item))
            .or_else(|| self.layout.first_visible());
        if self.selected_panel_index().is_none() {
            self.mode = match self.mode {
                AppMode::Fullscreen => AppMode::Normal,
                AppMode::FullscreenInspect => AppMode::Inspect,
                mode => mode,
            };
        }
        if self.mode == AppMode::Search {
            super::input::update_search_results(self);
        }
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

    async fn refresh_activation(&mut self, indices: &[usize]) {
        let range = self.range;
        let step_policy = self.step_policy;
        let scrape_interval = self.scrape_interval;
        let end_ts = self.view_end_ts;
        if self.variable_state.has_variables() {
            let active_scopes = self.active_variable_scopes();
            crate::app::variables::refresh_scoped_variables(
                &self.prometheus,
                &mut self.variable_state,
                &active_scopes,
                VariableRefreshContext {
                    update: VariableUpdate::Tick,
                    range,
                    intervals: QueryIntervals::for_policy(
                        range,
                        step_policy,
                        scrape_interval,
                        None,
                        None,
                    ),
                    end_ts,
                },
                &self.vars,
            )
            .await;
        }
        let mut requested = indices.to_vec();
        if self.dynamic.is_some() {
            requested.extend(self.reconcile_dynamic());
        }
        let eligible = self.query_eligible_indices();
        requested.retain(|i| eligible.contains(i));
        requested.sort_unstable();
        requested.dedup();
        let variable_report = Self::refresh_prometheus_data(
            &self.prometheus,
            &self.query_vars,
            &self.variable_state,
            self.dynamic.as_mut(),
            range,
            step_policy,
            scrape_interval,
            end_ts,
            &mut self.vars,
            &mut self.panels,
            &requested,
            &self.variable_errors,
            None,
        )
        .await;
        self.apply_variable_report(variable_report);
        self.evaluate_dynamic_visibility();
    }

    fn apply_variable_report(&mut self, report: VariableReport) {
        for name in report.queried {
            self.variable_errors.remove(&name);
        }
        self.variable_errors.extend(report.errors);
    }

    pub(crate) fn variable_warning(&self) -> Option<String> {
        let classic = self
            .variable_errors
            .iter()
            .map(|(name, error)| (name.as_str(), error.as_str()));
        let scoped = self.variable_state.scopes.iter().flat_map(|scope| {
            scope.variables.iter().filter_map(|variable| {
                variable
                    .last_error
                    .as_deref()
                    .map(|error| (variable.name.as_str(), error))
            })
        });
        let (name, error) = classic.chain(scoped).next()?;
        let error = error.lines().next().unwrap_or(error);
        Some(format!("Variable {name} failed: {error}"))
    }

    #[allow(clippy::too_many_arguments)]
    async fn refresh_prometheus_data(
        prometheus: &prom::PromClient,
        query_vars: &[TemplateQueryVar],
        variable_state: &crate::dashboard::variables::VariableState,
        mut dynamic: Option<&mut super::dynamic::DynamicDashboard>,
        range: Duration,
        step_policy: StepPolicy,
        scrape_interval: Duration,
        end_ts: i64,
        vars: &mut HashMap<String, String>,
        panels: &mut [PanelState],
        indices: &[usize],
        variable_errors: &BTreeMap<String, String>,
        variable_update: Option<VariableUpdate>,
    ) -> VariableReport {
        let variable_report = if let Some(variable_update) = variable_update
            && !variable_state.has_variables()
        {
            let failed_variables = variable_errors.keys().cloned().collect();
            refresh_query_variables(
                prometheus,
                query_vars,
                VariableRefreshContext {
                    update: variable_update,
                    range,
                    intervals: QueryIntervals::for_policy(
                        range,
                        step_policy,
                        scrape_interval,
                        None,
                        None,
                    ),
                    end_ts,
                },
                &failed_variables,
                vars,
            )
            .await
        } else {
            VariableReport::default()
        };

        // Create a stream of futures for fetching panel data.
        let indices = indices.iter().copied().collect::<HashSet<_>>();
        let work = panels
            .iter_mut()
            .enumerate()
            .filter_map(|(index, panel)| indices.contains(&index).then_some((index, panel)))
            .map(|(index, p)| {
                let values = dynamic
                    .as_ref()
                    .map(|d| d.values_for(index, variable_state, vars))
                    .unwrap_or_else(|| {
                        variable_state.scope_values(
                            variable_state.panel_scopes.get(index).copied().unwrap_or(0),
                            vars,
                        )
                    });
                (index, p, values)
            })
            .collect::<Vec<_>>();
        let mut futures = futures::stream::iter(work)
            .map(|(index, p, values)| async move {
                (
                    index,
                    Self::fetch_single_panel_data(
                        prometheus,
                        p,
                        range,
                        step_policy,
                        scrape_interval,
                        &values,
                        end_ts,
                    )
                    .await,
                )
            })
            .buffer_unordered(4);

        while let Some((index, (p, results, succeeded, url, errors, warnings))) =
            futures.next().await
        {
            let hidden = p
                .series
                .iter()
                .filter(|series| !series.visible)
                .map(|series| series.name.clone())
                .collect::<HashSet<_>>();
            let stale_queries = p
                .series
                .iter()
                .filter(|series| {
                    series.query_index < p.exprs.len() && !succeeded.contains(&series.query_index)
                })
                .map(|series| series.query_index)
                .collect::<HashSet<_>>()
                .len();
            let mut merged = results;
            merged.extend(
                p.series
                    .iter()
                    .filter(|series| {
                        series.query_index < p.exprs.len()
                            && !succeeded.contains(&series.query_index)
                    })
                    .cloned(),
            );
            merged.sort_by_key(|series| series.query_index);
            for series in &mut merged {
                series.visible = !hidden.contains(&series.name);
            }
            let has_data = merged.iter().any(|series| !series.points.is_empty());
            if let Some(dynamic) = dynamic.as_mut() {
                dynamic.mark_query_result(index, has_data);
            }
            p.series = merged;
            p.last_samples = p.series.iter().map(|s| s.points.len()).sum();
            if let Some(u) = url {
                p.last_url = Some(u);
            }
            p.last_error = (!errors.is_empty()).then(|| errors.join("\n"));
            p.notices = DataNotices {
                stale_queries,
                warnings,
            };
        }
        variable_report
    }

    async fn fetch_single_panel_data<'a>(
        prometheus: &prom::PromClient,
        p: &'a mut PanelState,
        range: Duration,
        step_policy: StepPolicy,
        scrape_interval: Duration,
        vars: &HashMap<String, String>,
        end_ts: i64,
    ) -> (
        &'a mut PanelState,
        Vec<SeriesView>,
        HashSet<usize>,
        Option<String>,
        Vec<String>,
        Vec<String>,
    ) {
        let mut panel_results = Vec::new();
        let mut succeeded = HashSet::new();
        let mut last_url = None;
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        for (i, expr) in p.exprs.iter().enumerate() {
            let intervals = p.query_intervals(i, range, step_policy, scrape_interval, vars);
            let step = intervals.step;
            let expr_expanded = expand_expr(expr, range, intervals, vars);
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
                Ok(result) => {
                    succeeded.insert(i);
                    for warning in result.warnings {
                        if !warnings.contains(&warning) {
                            warnings.push(warning);
                        }
                    }
                    for s in result.series {
                        let latest_val = match p.options {
                            PanelOptions::Summary(calc) => reduce_values(&s.values, calc),
                            _ => s.values.last().and_then(|(_, v)| v.parse::<f64>().ok()),
                        };
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
                            query_index: i,
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
                    errors.push(format!(
                        "{} failed for `{}`: {}",
                        query_name, expr_expanded, e
                    ));
                }
            }
        }
        (p, panel_results, succeeded, last_url, errors, warnings)
    }
}

/// Independent imported dashboards hosted in one terminal session.
const MAX_CONCURRENT_DASHBOARD_REFRESHES: usize = 4;

pub(crate) struct DashboardWorkspace {
    dashboards: Vec<AppState>,
    active: usize,
    generations: Vec<u64>,
    refresh_tasks: Vec<Option<DashboardRefreshTask>>,
    refresh_requested: Vec<Option<RefreshRequest>>,
    next_refresh_index: usize,
}

#[derive(Clone)]
enum RefreshRequest {
    Manual,
    Activation(Vec<usize>),
}

struct DashboardRefreshTask {
    request: Option<RefreshRequest>,
    handle: JoinHandle<RefreshCompletion>,
}

struct RefreshCompletion {
    generation: u64,
    previous_refresh: Instant,
    snapshot: AppState,
    annotation: Option<(
        Box<dyn crate::annotations::AnnotationProvider>,
        crate::annotations::ProviderPoll,
    )>,
    error: Option<String>,
}

impl DashboardWorkspace {
    pub(crate) fn new(dashboards: Vec<AppState>) -> Self {
        assert!(
            !dashboards.is_empty(),
            "a dashboard workspace cannot be empty"
        );
        let count = dashboards.len();
        Self {
            dashboards,
            active: 0,
            generations: vec![0; count],
            refresh_tasks: (0..count).map(|_| None).collect(),
            refresh_requested: vec![None; count],
            next_refresh_index: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn active(&self) -> &AppState {
        &self.dashboards[self.active]
    }

    pub(crate) fn active_mut(&mut self) -> &mut AppState {
        &mut self.dashboards[self.active]
    }

    pub(crate) fn active_index(&self) -> usize {
        self.active
    }

    pub(crate) fn dashboards(&self) -> &[AppState] {
        &self.dashboards
    }

    pub(crate) fn dashboard_mut(&mut self, index: usize) -> &mut AppState {
        &mut self.dashboards[index]
    }

    pub(crate) fn switch_next(&mut self) {
        self.active = (self.active + 1) % self.dashboards.len();
    }

    pub(crate) fn switch_previous(&mut self) {
        self.active = self
            .active
            .checked_sub(1)
            .unwrap_or(self.dashboards.len() - 1);
    }

    pub(crate) fn invalidate_active(&mut self) {
        self.generations[self.active] = self.generations[self.active].wrapping_add(1);
    }

    pub(crate) fn request_active_refresh(&mut self) {
        self.invalidate_active();
        self.refresh_requested[self.active] = Some(RefreshRequest::Manual);
    }

    pub(crate) fn request_activation_refresh(&mut self, indices: Vec<usize>) {
        self.invalidate_active();
        // A superseded manual refresh must retain its variable-refresh policy.
        let in_flight = self.refresh_tasks[self.active]
            .as_ref()
            .and_then(|task| task.request.clone());
        for request in in_flight
            .into_iter()
            .chain([RefreshRequest::Activation(indices)])
        {
            let pending = &mut self.refresh_requested[self.active];
            match (&mut *pending, request) {
                (Some(RefreshRequest::Manual), _) => {}
                (_, RefreshRequest::Manual) => *pending = Some(RefreshRequest::Manual),
                (Some(RefreshRequest::Activation(current)), RefreshRequest::Activation(next)) => {
                    current.extend(next);
                    current.sort_unstable();
                    current.dedup();
                }
                (None, next) => *pending = Some(next),
            }
        }
    }

    pub(crate) fn next_refresh_in(&self) -> Duration {
        if self
            .refresh_tasks
            .iter()
            .filter(|task| task.is_some())
            .count()
            >= MAX_CONCURRENT_DASHBOARD_REFRESHES
        {
            return Duration::from_millis(250);
        }
        self.dashboards
            .iter()
            .enumerate()
            .filter(|(index, _)| self.refresh_tasks[*index].is_none())
            .map(|(index, dashboard)| {
                if self.refresh_requested[index].is_some() {
                    return Duration::ZERO;
                }
                dashboard
                    .refresh_every
                    .saturating_sub(dashboard.last_refresh.elapsed())
            })
            .min()
            .unwrap_or(Duration::from_millis(250))
    }

    pub(crate) fn start_due_refreshes(&mut self) -> bool {
        let mut in_flight = self
            .refresh_tasks
            .iter()
            .filter(|task| task.is_some())
            .count();
        let mut started = false;
        let start_index = self.next_refresh_index;
        for offset in 0..self.dashboards.len() {
            let index = (start_index + offset) % self.dashboards.len();
            if in_flight >= MAX_CONCURRENT_DASHBOARD_REFRESHES {
                break;
            }
            let dashboard = &mut self.dashboards[index];
            if self.refresh_tasks[index].is_some()
                || (self.refresh_requested[index].is_none()
                    && dashboard.last_refresh.elapsed() < dashboard.refresh_every)
            {
                continue;
            }
            let request = self.refresh_requested[index].take();
            let update = if matches!(request, Some(RefreshRequest::Manual)) {
                VariableUpdate::TimeRangeChange
            } else {
                VariableUpdate::Tick
            };
            let activation = match &request {
                Some(RefreshRequest::Activation(indices)) if dashboard.variables_loaded => {
                    Some(indices.clone())
                }
                _ => None,
            };
            let end_ts = if activation.is_some() {
                dashboard.view_end_ts
            } else {
                chrono::Utc::now().timestamp() - dashboard.time_offset.as_secs() as i64
            };
            let context = crate::annotations::AnnotationRefreshContext::from_unix_window(
                end_ts,
                dashboard.range,
            );
            let provider = if activation.is_none() {
                dashboard.annotations.take_provider()
            } else {
                None
            };
            let mut snapshot = dashboard.refresh_snapshot();
            let generation = self.generations[index];
            let previous_refresh = dashboard.last_refresh;
            self.refresh_tasks[index] = Some(DashboardRefreshTask {
                request,
                handle: tokio::task::spawn_local(async move {
                    let annotation_refresh = async move {
                        match provider {
                            Some(mut provider) => {
                                let poll = provider.refresh(&context).await;
                                Some((provider, poll))
                            }
                            None => None,
                        }
                    };
                    let prometheus_refresh = async {
                        if let Some(indices) = activation {
                            snapshot.refresh_activation(&indices).await;
                            Ok(())
                        } else {
                            snapshot.refresh_at(update, end_ts).await
                        }
                    };
                    let (annotation, result) = tokio::join!(annotation_refresh, prometheus_refresh);
                    RefreshCompletion {
                        generation,
                        previous_refresh,
                        snapshot,
                        annotation,
                        error: result.err().map(|error| format!("{error:#}")),
                    }
                }),
            });
            in_flight += 1;
            started = true;
            self.next_refresh_index = (index + 1) % self.dashboards.len();
        }
        started
    }

    pub(crate) async fn collect_completed_refreshes(&mut self) -> Result<Vec<(usize, Instant)>> {
        let mut completed_indices = Vec::new();
        for index in 0..self.refresh_tasks.len() {
            if !self.refresh_tasks[index]
                .as_ref()
                .is_some_and(|task| task.handle.is_finished())
            {
                continue;
            }
            let task = self.refresh_tasks[index]
                .take()
                .expect("checked as present");
            let completion = task
                .handle
                .await
                .map_err(|error| anyhow::anyhow!("dashboard refresh task failed: {error}"))?;
            let dashboard = &mut self.dashboards[index];
            let current_generation = self.generations[index] == completion.generation;
            if let Some((provider, poll)) = completion.annotation {
                dashboard
                    .annotations
                    .restore_provider_after_refresh(provider, current_generation.then_some(poll));
            }
            if let Some(error) = completion.error {
                if current_generation {
                    dashboard.export_status = Some(format!("Dashboard refresh failed: {error}"));
                    dashboard.last_refresh = Instant::now();
                }
            } else if current_generation {
                dashboard.apply_refresh_snapshot(completion.snapshot);
                dashboard.reconcile_visible_annotation_targets();
            }
            completed_indices.push((index, completion.previous_refresh));
        }
        Ok(completed_indices)
    }

    pub(crate) fn finalize_recordings(&mut self) -> Result<()> {
        self.cancel_refreshes();
        for dashboard in &mut self.dashboards {
            super::event_loop::finalize_recording_before_quit(dashboard)?;
        }
        Ok(())
    }

    fn cancel_refreshes(&mut self) {
        for refresh in &mut self.refresh_tasks {
            if let Some(refresh) = refresh.take() {
                refresh.handle.abort();
            }
        }
    }
}

impl Drop for DashboardWorkspace {
    fn drop(&mut self) {
        self.cancel_refreshes();
    }
}

#[cfg(test)]
pub(super) mod test_support {
    use super::*;
    use crate::app::{event_loop, input::InputAction};

    pub(crate) fn loaded_fixture(mut app: AppState) -> AppState {
        app.variables_loaded = true;
        app
    }

    async fn complete(mut workspace: DashboardWorkspace) -> AppState {
        assert!(workspace.start_due_refreshes());
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if !workspace
                    .collect_completed_refreshes()
                    .await
                    .unwrap()
                    .is_empty()
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("requested dashboard refresh should finish");
        workspace.dashboards.remove(0)
    }

    pub(crate) async fn complete_activation(app: AppState, indices: Vec<usize>) -> AppState {
        tokio::task::LocalSet::new()
            .run_until(async {
                let mut workspace = DashboardWorkspace::new(vec![app]);
                workspace.request_activation_refresh(indices);
                complete(workspace).await
            })
            .await
    }

    pub(crate) async fn complete_event(
        app: AppState,
        event: crossterm::event::Event,
        viewport: ratatui::layout::Rect,
    ) -> AppState {
        tokio::task::LocalSet::new()
            .run_until(async {
                let mut workspace = DashboardWorkspace::new(vec![app]);
                let action = event_loop::handle_event(event, viewport, &mut workspace)
                    .await
                    .unwrap();
                if matches!(action, InputAction::Refresh | InputAction::Activate(_)) {
                    complete(workspace).await
                } else {
                    workspace.dashboards.remove(0)
                }
            })
            .await
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
    use tokio::sync::{Barrier, Notify};

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

    async fn workspace_regression_wait_for_workspace(workspace: &mut DashboardWorkspace) {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if !workspace
                    .collect_completed_refreshes()
                    .await
                    .unwrap()
                    .is_empty()
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("workspace refresh should finish");
    }

    #[tokio::test]
    async fn workspace_regression_workspace_manual_refresh_updates_range_variables() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (url, requests) = mock_prometheus(variable_values).await;
                let mut app = create_test_app();
                app.prometheus = prom::PromClient::new(url);
                app.query_vars = vec![
                    crate::grafana::TemplateQueryVar {
                        name: "range".into(),
                        query: "label_values(range)".into(),
                        regex: None,
                        query_path: "fixture.range".into(),
                        refresh: crate::grafana::VariableRefresh::OnTimeRangeChange,
                    },
                    crate::grafana::TemplateQueryVar {
                        name: "load".into(),
                        query: "label_values(load)".into(),
                        regex: None,
                        query_path: "fixture.load".into(),
                        refresh: crate::grafana::VariableRefresh::OnLoad,
                    },
                ];
                let mut workspace = DashboardWorkspace::new(vec![app]);
                assert!(workspace.start_due_refreshes());
                workspace_regression_wait_for_workspace(&mut workspace).await;
                workspace.active_mut().last_refresh = Instant::now() - Duration::from_secs(10);
                assert!(workspace.start_due_refreshes());
                workspace_regression_wait_for_workspace(&mut workspace).await;
                use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
                for key in ['r', '+'] {
                    super::super::event_loop::handle_event(
                        Event::Key(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE)),
                        ratatui::layout::Rect::new(0, 0, 80, 24),
                        &mut workspace,
                    )
                    .await
                    .unwrap();
                    assert!(workspace.start_due_refreshes());
                    workspace_regression_wait_for_workspace(&mut workspace).await;
                }
                let count = requests
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|request| request.starts_with("/api/v1/label/range/"))
                    .count();
                assert_eq!(
                    count, 3,
                    "initial, manual and range changes refresh variables; periodic ticks do not"
                );
                assert_eq!(
                    requests
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|request| request.starts_with("/api/v1/label/load/"))
                        .count(),
                    1
                );
            })
            .await;
    }

    #[tokio::test]
    async fn workspace_regression_row_expansion_returns_before_network_reply() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut app =
            row_test_app_with_queries(&format!("http://{}", listener.local_addr().unwrap()));
        let result = tokio::time::timeout(
            Duration::from_millis(100),
            crate::app::input::handle_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                ratatui::layout::Size::new(80, 24),
                &mut app,
            ),
        )
        .await;
        assert!(
            result.is_ok(),
            "row expansion blocks inside the input handler waiting for Prometheus"
        );
    }

    #[tokio::test]
    async fn workspace_regression_mouse_click_uses_rendered_row_position() {
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use ratatui::{Terminal, backend::TestBackend, layout::Rect};
        let mut app = row_test_app();
        app.apply_layout(row_test_layout(true));
        let mut workspace = DashboardWorkspace::new(vec![app, create_test_app()]);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| crate::ui::draw_workspace_ui(frame, &mut workspace))
            .unwrap();
        let row = crate::ui::visible_dashboard_rects(Rect::new(0, 1, 80, 23), workspace.active())
            .into_iter()
            .find(|item| item.id == DashboardItemId::Row(RowId::new(0)))
            .unwrap();
        let disclosure = row.disclosure_rect.unwrap();
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((row.rect.x, row.rect.y))
                .unwrap()
                .symbol(),
            "▶"
        );
        super::super::event_loop::handle_event(
            crossterm::event::Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: disclosure.x,
                row: disclosure.y,
                modifiers: KeyModifiers::NONE,
            }),
            Rect::new(0, 0, 80, 24),
            &mut workspace,
        )
        .await
        .unwrap();
        assert!(
            !workspace
                .active()
                .layout
                .row(RowId::new(0))
                .unwrap()
                .collapsed,
            "clicking the displayed row disclosure at {},{} must expand it",
            disclosure.x,
            disclosure.y
        );
    }

    #[tokio::test]
    async fn workspace_regression_workspace_refresh_reconciles_annotation_targets() {
        tokio::task::LocalSet::new().run_until(async {
            let path = temp_annotation_path("debug-audit-targets");
            tokio::fs::write(&path, "{\"time\":\"2026-08-12T10:00:00Z\",\"text\":\"deploy\",\"panel_titles\":[\"Missing\"]}\n").await.unwrap();
            let mut app = create_test_app();
            app.panels = vec![test_panel("CPU", PanelType::Graph)];
            app.apply_layout(DashboardLayout::flat(1));
            app.annotations = crate::annotations::AnnotationState::from_path(Some(path.clone()));
            let mut workspace = DashboardWorkspace::new(vec![app]);
            assert!(workspace.start_due_refreshes());
            workspace_regression_wait_for_workspace(&mut workspace).await;
            tokio::fs::remove_file(path).await.unwrap();
            let status = workspace.active().annotations.footer_status();
            assert!(status.as_deref().is_some_and(|text| text.contains("Missing")),
                "missing target diagnostic must survive workspace merge, got {status:?}");
        }).await;
    }

    #[tokio::test]
    async fn workspace_regression_noop_mouse_movement_does_not_discard_refresh() {
        use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
        tokio::task::LocalSet::new()
            .run_until(async {
                let release = Arc::new(Notify::new());
                let mut app = create_test_app();
                app.annotations = crate::annotations::AnnotationState::from_provider(Some(
                    Box::new(BlockedProvider(Arc::clone(&release))),
                ));
                let mut workspace = DashboardWorkspace::new(vec![app]);
                assert!(workspace.start_due_refreshes());
                super::super::event_loop::handle_event(
                    crossterm::event::Event::Mouse(MouseEvent {
                        kind: MouseEventKind::Moved,
                        column: 10,
                        row: 10,
                        modifiers: KeyModifiers::NONE,
                    }),
                    ratatui::layout::Rect::new(0, 0, 80, 24),
                    &mut workspace,
                )
                .await
                .unwrap();
                release.notify_one();
                workspace_regression_wait_for_workspace(&mut workspace).await;
                assert_eq!(
                    workspace.active().annotations.snapshot().unwrap().len(),
                    1,
                    "no-op mouse movement must not discard a valid refresh result"
                );
            })
            .await;
    }

    #[test]
    fn workspace_tab_switching_wraps_and_preserves_dashboard_state() {
        let mut alpha = create_test_app();
        alpha.title = "Alpha".into();
        alpha.range = Duration::from_secs(60);
        alpha.vars.insert("cluster".into(), "west".into());
        let mut beta = create_test_app();
        beta.title = "Beta".into();
        beta.range = Duration::from_secs(300);
        beta.vars.insert("cluster".into(), "east".into());
        let mut workspace = DashboardWorkspace::new(vec![alpha, beta]);

        assert_eq!(workspace.active_index(), 0);
        assert_eq!(workspace.active().title, "Alpha");
        workspace.switch_next();
        assert_eq!(workspace.active_index(), 1);
        assert_eq!(workspace.active().title, "Beta");
        workspace.active_mut().range = Duration::from_secs(900);
        workspace.switch_previous();
        assert_eq!(workspace.active_index(), 0);
        assert_eq!(workspace.active().range, Duration::from_secs(60));
        assert_eq!(workspace.active().vars["cluster"], "west");
        workspace.switch_previous();
        assert_eq!(workspace.active().title, "Beta");
        assert_eq!(workspace.active().range, Duration::from_secs(900));
        assert_eq!(workspace.active().vars["cluster"], "east");
    }

    #[tokio::test]
    async fn workspace_refresh_preserves_navigation_and_display_edits() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (url, requests) = mock_prometheus(successful_matrix).await;
                let release = Arc::new(Notify::new());
                let mut app = create_test_app();
                app.prometheus = prom::PromClient::new(url);
                app.panels = ["First", "Second"]
                    .into_iter()
                    .map(|title| {
                        let mut panel = test_panel(title, PanelType::Graph);
                        panel.exprs = vec!["up".into()];
                        panel.legends = vec![Some("api".into())];
                        panel.query_modes = vec![QueryMode::Range];
                        panel.series = vec![SeriesView {
                            query_index: 0,
                            name: "api".into(),
                            value: Some(0.0),
                            points: vec![],
                            visible: true,
                        }];
                        panel
                    })
                    .collect();
                app.apply_layout(DashboardLayout::flat(2));
                app.annotations = crate::annotations::AnnotationState::from_provider(Some(
                    Box::new(BlockedProvider(Arc::clone(&release))),
                ));
                let mut workspace = DashboardWorkspace::new(vec![app]);
                workspace.start_due_refreshes();
                tokio::time::timeout(Duration::from_secs(1), async {
                    while requests.lock().unwrap().is_empty() {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                let app = workspace.active_mut();
                app.selected_item = Some(DashboardItemId::Panel(1));
                app.mode = AppMode::Inspect;
                app.cursor_x = Some(123.0);
                app.debug_bar = true;
                app.panels[1].y_axis_mode = YAxisMode::ZeroBased;
                app.panels[1].series[0].visible = false;
                app.export_status = Some("latest export".into());
                release.notify_one();
                workspace_regression_wait_for_workspace(&mut workspace).await;
                let app = workspace.active();
                assert_eq!(app.panels[1].series[0].value, Some(2.0));
                assert_eq!(app.selected_item, Some(DashboardItemId::Panel(1)));
                assert_eq!(app.mode, AppMode::Inspect);
                assert_eq!(app.cursor_x, Some(123.0));
                assert!(app.debug_bar);
                assert_eq!(app.panels[1].y_axis_mode, YAxisMode::ZeroBased);
                assert!(!app.panels[1].series[0].visible);
                assert_eq!(app.export_status.as_deref(), Some("latest export"));
            })
            .await;
    }

    #[tokio::test]
    async fn workspace_activation_keeps_superseded_manual_refresh_policy() {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
        tokio::task::LocalSet::new()
            .run_until(async {
                let (url, requests) = mock_prometheus(variable_values).await;
                let mut app = create_test_app();
                app.prometheus = prom::PromClient::new(url);
                app.query_vars = vec![TemplateQueryVar {
                    name: "range".into(),
                    query: "label_values(range)".into(),
                    regex: None,
                    query_path: "fixture.range".into(),
                    refresh: crate::grafana::VariableRefresh::OnTimeRangeChange,
                }];
                app.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::Row(
                    DashboardRow::new(RowId::new(0), "Empty row", true, false, vec![]),
                )]));
                app.refresh_initial().await.unwrap();
                let release = Arc::new(Notify::new());
                app.annotations = crate::annotations::AnnotationState::from_provider(Some(
                    Box::new(BlockedProvider(Arc::clone(&release))),
                ));
                let mut workspace = DashboardWorkspace::new(vec![app]);
                let viewport = ratatui::layout::Rect::new(0, 0, 80, 24);
                super::super::event_loop::handle_event(
                    Event::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)),
                    viewport,
                    &mut workspace,
                )
                .await
                .unwrap();
                assert!(workspace.start_due_refreshes());
                tokio::time::timeout(Duration::from_secs(1), async {
                    while requests.lock().unwrap().len() < 2 {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                super::super::event_loop::handle_event(
                    Event::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
                    viewport,
                    &mut workspace,
                )
                .await
                .unwrap();
                release.notify_one();
                workspace_regression_wait_for_workspace(&mut workspace).await;
                assert!(workspace.start_due_refreshes());
                release.notify_one();
                workspace_regression_wait_for_workspace(&mut workspace).await;
                assert_eq!(
                    requests
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|request| request.starts_with("/api/v1/label/range/"))
                        .count(),
                    3,
                    "activation must not downgrade the pending manual request to a tick"
                );
            })
            .await;
    }

    #[tokio::test]
    async fn workspace_refresh_maps_live_selection_to_repeated_panel_identity() {
        use crate::dashboard::{
            autogrid::{AutoGridBehavior, DashboardAutoGrid, test_items},
            variables::{Variable, VariableOption},
        };
        fn changed_node(target: &str) -> &'static str {
            if target.starts_with("/api/v1/label/") {
                r#"{"status":"success","data":["b"]}"#
            } else {
                successful_matrix(target)
            }
        }
        tokio::task::LocalSet::new()
            .run_until(async {
                let (url, requests) = mock_prometheus(changed_node).await;
                let mut app = create_test_app();
                app.prometheus = prom::PromClient::new(url);
                let mut panel = test_panel("Load $node", PanelType::Graph);
                panel.exprs = vec!["up{node=\"$node\"}".into()];
                panel.legends = vec![Some("api".into())];
                panel.query_modes = vec![QueryMode::Range];
                app.panels = vec![panel];
                app.variable_state.scopes[0].variables = vec![Variable {
                    name: "node".into(),
                    values: vec!["a".into(), "b".into()],
                    texts: vec!["a".into(), "b".into()],
                    options: ["a", "b"]
                        .into_iter()
                        .map(|v| VariableOption {
                            value: v.into(),
                            text: v.into(),
                        })
                        .collect(),
                    all: false,
                    all_value: None,
                    repeatable: true,
                    last_error: None,
                    resolved: true,
                    query: Some(TemplateQueryVar {
                        name: "node".into(),
                        query: "label_values(node)".into(),
                        regex: None,
                        query_path: "fixture.node".into(),
                        refresh: crate::grafana::VariableRefresh::OnTimeRangeChange,
                    }),
                }];
                app.variable_state.panel_scopes = vec![0];
                app.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::AutoGrid(
                    DashboardAutoGrid {
                        options: Default::default(),
                        items: test_items(vec![0]),
                    },
                )]));
                app.configure_dynamic(HashMap::from([(
                    0,
                    AutoGridBehavior {
                        repeat: Some("node".into()),
                        ..Default::default()
                    },
                )]));
                app.variables_loaded = true;
                let release = Arc::new(Notify::new());
                app.annotations = crate::annotations::AnnotationState::from_provider(Some(
                    Box::new(BlockedProvider(Arc::clone(&release))),
                ));
                let mut workspace = DashboardWorkspace::new(vec![app]);
                workspace.request_active_refresh();
                workspace.start_due_refreshes();
                tokio::time::timeout(Duration::from_secs(1), async {
                    while requests.lock().unwrap().len() < 2 {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                workspace.active_mut().selected_item = Some(DashboardItemId::Panel(1));
                workspace.active_mut().panels[1].y_axis_mode = YAxisMode::ZeroBased;
                release.notify_one();
                workspace_regression_wait_for_workspace(&mut workspace).await;
                assert_eq!(workspace.active().panels.len(), 1);
                assert_eq!(workspace.active().panels[0].title, "Load b");
                assert_eq!(workspace.active().selected_panel_index(), Some(0));
                assert_eq!(
                    workspace.active().panels[0].y_axis_mode,
                    YAxisMode::ZeroBased
                );
            })
            .await;
    }

    #[derive(Debug)]
    struct DashboardRefreshBarrier(Arc<Barrier>);

    impl AnnotationProvider for DashboardRefreshBarrier {
        fn refresh<'a>(&'a mut self, _context: &'a AnnotationRefreshContext) -> ProviderFuture<'a> {
            let barrier = Arc::clone(&self.0);
            Box::pin(async move {
                barrier.wait().await;
                ProviderPoll::Loaded(AnnotationSnapshot::new(Vec::new()))
            })
        }
    }

    #[tokio::test]
    async fn workspace_refreshes_inactive_dashboards_concurrently() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let barrier = Arc::new(Barrier::new(2));
                let mut alpha = create_test_app();
                alpha.title = "Alpha".into();
                alpha.annotations = crate::annotations::AnnotationState::from_provider(Some(
                    Box::new(DashboardRefreshBarrier(Arc::clone(&barrier))),
                ));
                let mut beta = create_test_app();
                beta.title = "Beta".into();
                beta.annotations = crate::annotations::AnnotationState::from_provider(Some(
                    Box::new(DashboardRefreshBarrier(Arc::clone(&barrier))),
                ));
                let mut workspace = DashboardWorkspace::new(vec![alpha, beta]);

                workspace.start_due_refreshes();
                tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        if workspace.collect_completed_refreshes().await.unwrap().len() == 2 {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .expect("dashboard refreshes must run concurrently");

                assert!(workspace.dashboards()[0].last_refresh.elapsed() < Duration::from_secs(1));
                assert!(workspace.dashboards()[1].last_refresh.elapsed() < Duration::from_secs(1));
                assert!(workspace.dashboards()[0].annotations.snapshot().is_some());
                assert!(workspace.dashboards()[1].annotations.snapshot().is_some());
            })
            .await;
    }

    #[tokio::test]
    async fn workspace_caps_background_dashboard_refreshes() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let dashboards = (0..MAX_CONCURRENT_DASHBOARD_REFRESHES + 2)
                    .map(|index| {
                        let mut dashboard = create_test_app();
                        dashboard.title = format!("Dashboard {index}");
                        dashboard
                    })
                    .collect();
                let mut workspace = DashboardWorkspace::new(dashboards);

                assert!(workspace.start_due_refreshes());
                assert_eq!(
                    workspace
                        .refresh_tasks
                        .iter()
                        .filter(|task| task.is_some())
                        .count(),
                    MAX_CONCURRENT_DASHBOARD_REFRESHES
                );
                assert!(!workspace.next_refresh_in().is_zero());
                assert!(!workspace.start_due_refreshes());
            })
            .await;
    }

    #[tokio::test]
    async fn requested_refresh_starts_without_waiting_for_the_periodic_interval() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let mut dashboard = create_test_app();
                dashboard.last_refresh = Instant::now();
                let mut workspace = DashboardWorkspace::new(vec![dashboard]);

                workspace.request_active_refresh();

                assert!(workspace.start_due_refreshes());
                assert!(workspace.refresh_tasks[0].is_some());
            })
            .await;
    }

    #[tokio::test]
    async fn requested_refresh_does_not_overlap_an_in_flight_dashboard_refresh() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let mut workspace = DashboardWorkspace::new(vec![create_test_app()]);
                assert!(workspace.start_due_refreshes());

                workspace.request_active_refresh();

                assert!(!workspace.start_due_refreshes());
                assert_eq!(
                    workspace
                        .refresh_tasks
                        .iter()
                        .filter(|task| task.is_some())
                        .count(),
                    1
                );
            })
            .await;
    }

    #[tokio::test]
    async fn requested_refresh_runs_after_the_in_flight_refresh_is_discarded() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let release = Arc::new(Notify::new());
                let mut dashboard = create_test_app();
                dashboard.annotations = crate::annotations::AnnotationState::from_provider(Some(
                    Box::new(BlockedProvider(Arc::clone(&release))),
                ));
                let mut workspace = DashboardWorkspace::new(vec![dashboard]);
                assert!(workspace.start_due_refreshes());
                workspace.invalidate_active();
                workspace.request_active_refresh();
                release.notify_one();

                tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        if !workspace
                            .collect_completed_refreshes()
                            .await
                            .unwrap()
                            .is_empty()
                        {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .expect("the original refresh should complete");

                assert!(workspace.start_due_refreshes());
                assert!(workspace.refresh_tasks[0].is_some());
            })
            .await;
    }

    #[tokio::test]
    async fn zero_interval_refresh_scheduling_rotates_past_the_first_batch() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let dashboards = (0..MAX_CONCURRENT_DASHBOARD_REFRESHES + 2)
                    .map(|index| {
                        let mut dashboard = create_test_app();
                        dashboard.title = format!("Dashboard {index}");
                        dashboard.refresh_every = Duration::ZERO;
                        dashboard
                    })
                    .collect();
                let mut workspace = DashboardWorkspace::new(dashboards);

                workspace.start_due_refreshes();
                tokio::task::yield_now().await;
                workspace.collect_completed_refreshes().await.unwrap();
                workspace.start_due_refreshes();

                assert!(workspace.refresh_tasks[4].is_some());
                assert!(workspace.refresh_tasks[5].is_some());
            })
            .await;
    }

    #[tokio::test]
    async fn stale_refresh_failure_does_not_replace_newer_dashboard_status() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let mut dashboard = create_test_app();
                dashboard.export_status = Some("newer status".into());
                dashboard.last_refresh = Instant::now() - Duration::from_secs(5);
                let last_refresh = dashboard.last_refresh;
                let mut workspace = DashboardWorkspace::new(vec![dashboard]);
                let snapshot = workspace.dashboards[0].refresh_snapshot();
                workspace.generations[0] = 1;
                workspace.refresh_tasks[0] = Some(DashboardRefreshTask {
                    request: None,
                    handle: tokio::task::spawn_local(async move {
                        RefreshCompletion {
                            generation: 0,
                            previous_refresh: last_refresh,
                            snapshot,
                            annotation: None,
                            error: Some("outdated failure".into()),
                        }
                    }),
                });
                tokio::task::yield_now().await;

                workspace.collect_completed_refreshes().await.unwrap();

                assert_eq!(
                    workspace.dashboards[0].export_status.as_deref(),
                    Some("newer status")
                );
                assert_eq!(workspace.dashboards[0].last_refresh, last_refresh);
            })
            .await;
    }

    #[derive(Debug)]
    struct BlockedProvider(Arc<Notify>);

    impl AnnotationProvider for BlockedProvider {
        fn refresh<'a>(&'a mut self, _context: &'a AnnotationRefreshContext) -> ProviderFuture<'a> {
            let release = Arc::clone(&self.0);
            Box::pin(async move {
                release.notified().await;
                ProviderPoll::Loaded(AnnotationSnapshot::new(vec![
                    crate::annotations::test_event_at(0.0, "stale"),
                ]))
            })
        }
    }

    #[tokio::test]
    async fn workspace_discards_refresh_results_made_stale_by_input() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let release = Arc::new(Notify::new());
                let mut alpha = create_test_app();
                alpha.range = Duration::from_secs(60);
                alpha.annotations = crate::annotations::AnnotationState::from_provider(Some(
                    Box::new(BlockedProvider(Arc::clone(&release))),
                ));
                let mut workspace = DashboardWorkspace::new(vec![alpha]);

                assert!(workspace.start_due_refreshes());
                workspace.active_mut().range = Duration::from_secs(900);
                workspace.invalidate_active();
                release.notify_one();

                tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        if !workspace
                            .collect_completed_refreshes()
                            .await
                            .unwrap()
                            .is_empty()
                        {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .expect("the released refresh should finish");

                assert_eq!(workspace.active().range, Duration::from_secs(900));
                assert_eq!(workspace.active().annotations.snapshot().unwrap().len(), 0);
            })
            .await;
    }

    fn test_panel(title: &str, panel_type: PanelType) -> PanelState {
        PanelState {
            title: title.to_string(),
            exprs: vec![],
            legends: vec![],
            query_modes: vec![],
            series: vec![],
            last_error: None,
            notices: Default::default(),
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
            resolution: Default::default(),
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

    /// Serves Prometheus API requests from `respond` until the test ends,
    /// recording each request target.
    async fn mock_prometheus(
        respond: fn(&str) -> &'static str,
    ) -> (String, Arc<std::sync::Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
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
                    let target = request.split_whitespace().nth(1).unwrap_or("").to_string();
                    let body = respond(&target);
                    recorded.lock().unwrap().push(target);
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body,
                    );
                    socket.write_all(response.as_bytes()).await.unwrap();
                });
            }
        });
        (format!("http://{address}"), requests)
    }

    fn successful_matrix(_: &str) -> &'static str {
        r#"{"status":"success","data":{"resultType":"matrix","result":[{"metric":{"job":"api"},"values":[[1700000000,"1"],[1700000060,"2"]]}]}}"#
    }

    fn reduction_matrix(_: &str) -> &'static str {
        r#"{"status":"success","data":{"resultType":"matrix","result":[{"metric":{},"values":[[1700000000,"2"],[1700000060,"4"],[1700000120,"NaN"]]}]}}"#
    }

    #[tokio::test]
    async fn summary_options_reduce_raw_series_values() {
        let (url, _) = mock_prometheus(reduction_matrix).await;
        let mut app = create_test_app();
        app.prometheus = prom::PromClient::new(url);
        let mut panel = test_panel("Summary", PanelType::Gauge);
        panel.exprs = vec!["up".to_string()];
        panel.legends = vec![None];
        panel.query_modes = vec![QueryMode::Range];
        panel.options = PanelOptions::Summary(SummaryCalc::Mean);
        app.panels = vec![panel];
        app.apply_layout(DashboardLayout::flat(1));

        for (calc, expected) in [
            (SummaryCalc::Last, None),
            (SummaryCalc::LastNotNull, Some(4.0)),
            (SummaryCalc::Min, Some(2.0)),
            (SummaryCalc::Max, Some(4.0)),
            (SummaryCalc::Mean, Some(3.0)),
            (SummaryCalc::Sum, Some(6.0)),
        ] {
            app.panels[0].options = PanelOptions::Summary(calc);
            app.refresh().await.unwrap();
            assert_eq!(app.panels[0].series[0].value, expected, "{calc:?}");
            assert_eq!(app.panels[0].series[0].points.len(), 2);
        }
    }

    #[test]
    fn summary_reduction_handles_empty_and_long_raw_series() {
        let empty = vec![(0.0, "NaN".to_string()), (1.0, "bad".to_string())];
        for calc in [
            SummaryCalc::Last,
            SummaryCalc::LastNotNull,
            SummaryCalc::Min,
            SummaryCalc::Max,
            SummaryCalc::Mean,
            SummaryCalc::Sum,
        ] {
            assert_eq!(reduce_values(&empty, calc), None, "{calc:?}");
        }
        let raw: Vec<_> = (0..201)
            .map(|index| (index as f64, (index + 1).to_string()))
            .collect();
        assert_eq!(reduce_values(&raw, SummaryCalc::Mean), Some(101.0));
    }

    fn variable_values(target: &str) -> &'static str {
        if target.starts_with("/api/v1/label/") {
            r#"{"status":"success","data":["api"]}"#
        } else {
            r#"{"status":"success","data":{"resultType":"matrix","result":[]}}"#
        }
    }

    #[tokio::test]
    async fn app_refreshes_variables_for_initial_manual_and_periodic_triggers() {
        let (url, requests) = mock_prometheus(variable_values).await;
        let mut app = create_test_app();
        app.prometheus = prom::PromClient::new(url);
        app.query_vars = vec![
            crate::grafana::TemplateQueryVar {
                name: "load".into(),
                query: "label_values(load)".into(),
                regex: None,
                query_path: "fixture.load".into(),
                refresh: crate::grafana::VariableRefresh::OnLoad,
            },
            crate::grafana::TemplateQueryVar {
                name: "range".into(),
                query: "label_values(range)".into(),
                regex: None,
                query_path: "fixture.range".into(),
                refresh: crate::grafana::VariableRefresh::OnTimeRangeChange,
            },
        ];

        app.refresh_initial().await.unwrap();
        app.refresh_periodic().await.unwrap();
        app.refresh().await.unwrap();

        let requests = requests.lock().unwrap();
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.starts_with("/api/v1/label/load/"))
                .count(),
            1
        );
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.starts_with("/api/v1/label/range/"))
                .count(),
            2
        );
    }

    #[tokio::test]
    async fn manual_refresh_retries_failed_load_variable_and_clears_warning() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut app = create_test_app();
        app.prometheus =
            prom::PromClient::new(format!("http://{}", listener.local_addr().unwrap()));
        app.vars.insert("instance".into(), "node-a".into());
        app.query_vars = vec![crate::grafana::TemplateQueryVar {
            name: "instance".into(),
            query: "label_values(instance)".into(),
            regex: None,
            query_path: "fixture.instance".into(),
            refresh: crate::grafana::VariableRefresh::OnLoad,
        }];
        let server = tokio::spawn(async move {
            for body in [
                r#"{"status":"error","data":[],"errorType":"timeout","error":"request timed out"}"#,
                r#"{"status":"success","data":["node-b"]}"#,
            ] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0_u8; 4096];
                let _ = socket.read(&mut request).await.unwrap();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });

        app.refresh_initial().await.unwrap();
        assert_eq!(app.vars["instance"], "node-a");
        assert!(
            app.variable_warning()
                .is_some_and(|warning| warning.starts_with("Variable instance failed:"))
        );

        app.refresh_periodic().await.unwrap();
        assert_eq!(app.vars["instance"], "node-a");
        assert!(app.variable_warning().is_some());

        app.refresh().await.unwrap();
        assert_eq!(app.vars["instance"], "node-b");
        assert!(app.variable_warning().is_none());
        server.await.unwrap();
    }

    #[test]
    fn variable_report_clears_only_errors_for_variables_queried_again() {
        let mut app = create_test_app();
        app.variable_errors
            .insert("cluster".into(), "old cluster failure".into());
        app.variable_errors
            .insert("instance".into(), "old instance failure".into());

        app.apply_variable_report(crate::app::variables::VariableReport {
            queried: vec!["instance".into(), "job".into()],
            errors: vec![("job".into(), "new job failure".into())],
        });

        assert_eq!(
            app.variable_errors.get("cluster").map(String::as_str),
            Some("old cluster failure")
        );
        assert!(!app.variable_errors.contains_key("instance"));
        assert_eq!(
            app.variable_errors.get("job").map(String::as_str),
            Some("new job failure")
        );
    }

    fn rejected_query(_: &str) -> &'static str {
        r#"{"status":"error","errorType":"bad_data","error":"bad expression"}"#
    }

    fn initial_two_query_results(target: &str) -> &'static str {
        if target.contains("query=first") {
            r#"{"status":"success","data":{"resultType":"matrix","result":[{"metric":{},"values":[[1700000000,"1"]]}]}}"#
        } else {
            r#"{"status":"success","data":{"resultType":"matrix","result":[{"metric":{},"values":[[1700000000,"2"]]}]}}"#
        }
    }

    fn partial_two_query_results(target: &str) -> &'static str {
        if target.contains("query=first") {
            r#"{"status":"success","data":{"resultType":"matrix","result":[{"metric":{},"values":[[1700000060,"10"]]}]}}"#
        } else {
            rejected_query(target)
        }
    }

    fn warning_matrix(_: &str) -> &'static str {
        r#"{"status":"success","data":{"resultType":"matrix","result":[{"metric":{},"values":[[1700000000,"1"]]}]},"warnings":["partial response"]}"#
    }

    fn clean_matrix(_: &str) -> &'static str {
        r#"{"status":"success","data":{"resultType":"matrix","result":[{"metric":{},"values":[[1700000060,"2"]]}]}}"#
    }

    #[tokio::test]
    async fn failed_refresh_keeps_the_last_successful_series() {
        let (success_url, _) = mock_prometheus(successful_matrix).await;
        let mut app = create_test_app();
        app.prometheus = prom::PromClient::new(success_url);
        let mut panel = test_panel("Requests", PanelType::Graph);
        panel.exprs = vec!["up".to_string()];
        panel.legends = vec![None];
        panel.query_modes = vec![QueryMode::Range];
        app.panels = vec![panel];
        app.apply_layout(DashboardLayout::flat(1));

        app.refresh().await.unwrap();
        assert_eq!(app.panels[0].series.len(), 1);
        let expected_name = app.panels[0].series[0].name.clone();
        let expected_value = app.panels[0].series[0].value;
        let expected_points = app.panels[0].series[0].points.clone();
        assert_eq!(app.panels[0].last_samples, 2);

        let (failure_url, _) = mock_prometheus(rejected_query).await;
        app.prometheus = prom::PromClient::new(failure_url);
        app.refresh().await.unwrap();

        assert_eq!(app.panels[0].series.len(), 1);
        assert_eq!(app.panels[0].series[0].name, expected_name);
        assert_eq!(app.panels[0].series[0].value, expected_value);
        assert_eq!(app.panels[0].series[0].points, expected_points);
        assert_eq!(app.panels[0].last_samples, 2);
        assert_eq!(app.panels[0].notices.stale_queries, 1);
        assert!(
            app.panels[0]
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("bad expression"))
        );
    }

    #[tokio::test]
    async fn partial_refresh_replaces_successful_query_and_keeps_failed_query_series() {
        let (initial_url, _) = mock_prometheus(initial_two_query_results).await;
        let mut app = create_test_app();
        app.prometheus = prom::PromClient::new(initial_url);
        let mut panel = test_panel("Requests", PanelType::Graph);
        panel.exprs = vec!["first".to_string(), "second".to_string()];
        panel.legends = vec![Some("first".to_string()), Some("second".to_string())];
        panel.query_modes = vec![QueryMode::Range, QueryMode::Range];
        app.panels = vec![panel];
        app.apply_layout(DashboardLayout::flat(1));

        app.refresh().await.unwrap();
        assert_eq!(
            app.panels[0]
                .series
                .iter()
                .map(|series| (series.name.as_str(), series.value))
                .collect::<Vec<_>>(),
            [("first", Some(1.0)), ("second", Some(2.0))]
        );

        let (partial_url, _) = mock_prometheus(partial_two_query_results).await;
        app.prometheus = prom::PromClient::new(partial_url);
        app.refresh().await.unwrap();

        assert_eq!(
            app.panels[0]
                .series
                .iter()
                .map(|series| (series.name.as_str(), series.value))
                .collect::<Vec<_>>(),
            [("first", Some(10.0)), ("second", Some(2.0))]
        );
        assert_eq!(app.panels[0].notices.stale_queries, 1);
        assert!(
            app.panels[0]
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("second") && error.contains("bad expression"))
        );
    }

    #[tokio::test]
    async fn prometheus_warnings_follow_their_result_and_clear_on_clean_success() {
        let (warning_url, _) = mock_prometheus(warning_matrix).await;
        let mut app = create_test_app();
        app.prometheus = prom::PromClient::new(warning_url);
        let mut panel = test_panel("Requests", PanelType::Graph);
        panel.exprs = vec!["up".to_string()];
        panel.legends = vec![None];
        panel.query_modes = vec![QueryMode::Range];
        app.panels = vec![panel];
        app.apply_layout(DashboardLayout::flat(1));

        app.refresh().await.unwrap();
        assert_eq!(app.panels[0].notices.warnings, ["partial response"]);
        assert_eq!(app.panels[0].notices.stale_queries, 0);

        let (clean_url, _) = mock_prometheus(clean_matrix).await;
        app.prometheus = prom::PromClient::new(clean_url);
        app.refresh().await.unwrap();

        assert!(app.panels[0].notices.warnings.is_empty());
        assert_eq!(app.panels[0].notices.stale_queries, 0);
        assert!(app.panels[0].last_error.is_none());
    }

    #[tokio::test]
    async fn failed_refresh_reports_every_failed_query() {
        let (failure_url, _) = mock_prometheus(rejected_query).await;
        let mut app = create_test_app();
        app.prometheus = prom::PromClient::new(failure_url);
        let mut panel = test_panel("Requests", PanelType::Graph);
        panel.exprs = vec!["first".to_string(), "second".to_string()];
        panel.legends = vec![None, None];
        panel.query_modes = vec![QueryMode::Range, QueryMode::Range];
        app.panels = vec![panel];
        app.apply_layout(DashboardLayout::flat(1));

        app.refresh().await.unwrap();

        let error = app.panels[0].last_error.as_deref().unwrap();
        assert!(error.contains("`first`"), "{error}");
        assert!(error.contains("`second`"), "{error}");
        assert_eq!(error.lines().count(), 2, "{error}");
    }

    #[tokio::test]
    async fn refresh_drops_series_for_queries_removed_from_the_panel() {
        let (initial_url, _) = mock_prometheus(initial_two_query_results).await;
        let mut app = create_test_app();
        app.prometheus = prom::PromClient::new(initial_url);
        let mut panel = test_panel("Requests", PanelType::Graph);
        panel.exprs = vec!["first".to_string(), "second".to_string()];
        panel.legends = vec![Some("first".to_string()), Some("second".to_string())];
        panel.query_modes = vec![QueryMode::Range, QueryMode::Range];
        app.panels = vec![panel];
        app.apply_layout(DashboardLayout::flat(1));

        app.refresh().await.unwrap();
        app.panels[0].exprs.truncate(1);
        app.panels[0].legends.truncate(1);
        app.panels[0].query_modes.truncate(1);

        let (refresh_url, _) = mock_prometheus(initial_two_query_results).await;
        app.prometheus = prom::PromClient::new(refresh_url);
        app.refresh().await.unwrap();

        assert_eq!(
            app.panels[0]
                .series
                .iter()
                .map(|series| series.name.as_str())
                .collect::<Vec<_>>(),
            ["first"]
        );
        assert_eq!(app.panels[0].notices.stale_queries, 0);
    }

    #[tokio::test]
    async fn refresh_skips_collapsed_panels_and_expand_fetches_only_newly_visible() {
        let mut app = row_test_app_with_queries("http://127.0.0.1:9");

        app.refresh().await.unwrap();
        assert!(app.panels[0].last_error.is_none());
        assert!(app.panels[1].last_error.is_none());

        let indices = app.toggle_selected_row().await.unwrap().unwrap();
        app = test_support::complete_activation(app, indices).await;
        assert!(app.panels[0].last_error.is_some());
        assert!(app.panels[1].last_error.is_none());
    }

    #[tokio::test]
    async fn expansion_uses_the_displayed_window_without_resetting_refresh_clock() {
        let mut app = row_test_app_with_queries("http://127.0.0.1:9");
        app.view_end_ts = 1_700_000_000;
        app.variables_loaded = true;
        app.last_refresh = Instant::now() - Duration::from_secs(30);
        let last_refresh = app.last_refresh;

        let indices = app.toggle_selected_row().await.unwrap().unwrap();
        app = test_support::complete_activation(app, indices).await;

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
            notices: Default::default(),
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
            resolution: Default::default(),
        };

        assert_eq!(panel.query_mode(0), QueryMode::Instant);
        assert_eq!(panel.query_mode(1), QueryMode::Range);
    }

    fn range_panel(title: &str, exprs: &[&str]) -> PanelState {
        let mut panel = test_panel(title, PanelType::Graph);
        panel.exprs = exprs.iter().map(|expr| expr.to_string()).collect();
        panel.legends = vec![None; exprs.len()];
        panel.query_modes = vec![QueryMode::Range; exprs.len()];
        panel
    }

    #[test]
    fn target_min_interval_overrides_panel_minimum_in_automatic_mode() {
        let mut panel = range_panel("Resolution", &["a", "b", "c"]);
        panel.resolution = QueryResolution {
            min_interval: Some(">2m".to_string()),
            max_data_points: None,
            target_min_intervals: vec![Some("30s".to_string()), None, Some("$step".to_string())],
        };
        let vars = HashMap::from([("step".to_string(), "10m".to_string())]);
        let range = Duration::from_secs(3600);
        let step_policy = super::StepPolicy::Automatic {
            min_step: Duration::from_secs(5),
        };
        let scrape = Duration::from_secs(15);
        let intervals = |index| panel.query_intervals(index, range, step_policy, scrape, &vars);

        // The target's 30s also stands in for the scrape interval.
        assert_eq!(intervals(0).step, Duration::from_secs(30));
        assert_eq!(intervals(0).rate_interval, Duration::from_secs(120));
        assert_eq!(intervals(1).step, Duration::from_secs(120));
        assert_eq!(intervals(2).step, Duration::from_secs(600));

        // An unresolved variable falls back to the panel minimum.
        let intervals = panel.query_intervals(2, range, step_policy, scrape, &HashMap::new());
        assert_eq!(intervals.step, Duration::from_secs(120));
    }

    #[test]
    fn explicit_step_overrides_imported_minimums() {
        let mut panel = range_panel("Resolution", &["a"]);
        panel.resolution = QueryResolution {
            min_interval: Some("2m".to_string()),
            max_data_points: Some(10),
            target_min_intervals: vec![Some("30s".to_string())],
        };

        let intervals = panel.query_intervals(
            0,
            Duration::from_secs(24 * 3600),
            super::StepPolicy::Explicit(Duration::from_secs(7)),
            Duration::from_secs(15),
            &HashMap::new(),
        );

        assert_eq!(intervals.step, Duration::from_secs(7));
    }

    #[tokio::test]
    async fn refresh_requests_steps_scaled_to_the_range() {
        let (url, requests) = mock_prometheus(
            |_| r#"{"status":"success","data":{"resultType":"matrix","result":[]}}"#,
        )
        .await;
        let mut sparse = range_panel("Sparse", &["sparse"]);
        sparse.resolution.max_data_points = Some(60);
        let mut tuned = range_panel("Tuned", &["rate(tuned[$__rate_interval])"]);
        tuned.resolution.target_min_intervals = vec![Some("10m".to_string())];
        let mut app = AppState::new(
            prom::PromClient::new(url),
            Duration::from_secs(24 * 3600),
            super::StepPolicy::Automatic {
                min_step: Duration::from_secs(5),
            },
            Duration::from_millis(1000),
            "Test".to_string(),
            vec![range_panel("Default", &["default"]), sparse, tuned],
            0,
            Theme::default(),
            "dashed".to_string(),
            ExportOptions::default(),
        );

        app.refresh().await.unwrap();

        let requests = requests.lock().unwrap().clone();
        let step_for = |metric: &str| {
            let request = requests
                .iter()
                .map(|request| urlencoding::decode(request).unwrap().into_owned())
                .find(|request| request.contains(&format!("query={metric}")))
                .unwrap_or_else(|| panic!("no request for {metric} in {requests:?}"));
            let step = request.split("step=").nth(1).unwrap().to_string();
            (request, step)
        };
        // 24h / 1000 points = 86.4s, rounded to 1m.
        assert_eq!(step_for("default").1, "60s");
        // 24h / 60 points = 24m, rounded to 20m.
        assert_eq!(step_for("sparse").1, "1200s");
        // The 10m min interval sets the step and max(10m + 10m, 4 * 10m).
        let (request, step) = step_for("rate(tuned");
        assert_eq!(step, "600s");
        assert!(request.contains("rate(tuned[2400s])"), "{request}");
        assert_eq!(app.default_intervals().step, Duration::from_secs(60));
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
            notices: Default::default(),
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
            resolution: Default::default(),
        };

        assert_eq!(panel.graph_options(), GraphOptions::default());
    }
}
