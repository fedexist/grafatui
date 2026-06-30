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

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Result of importing a Grafana dashboard.
#[derive(Debug, Clone, Default)]
pub(crate) struct DashboardImport {
    /// Dashboard title.
    pub(crate) title: String,
    /// List of panels extracted.
    pub(crate) queries: Vec<QueryPanel>,
    /// Variables extracted from `templating.list`.
    pub(crate) vars: HashMap<String, String>,
    /// Dynamic query variables extracted from `templating.list`.
    pub(crate) query_vars: Vec<TemplateQueryVar>,
    /// Number of panels that were skipped (unsupported types).
    pub(crate) skipped_panels: usize,
    /// Dashboard-level refresh interval in milliseconds, if provided.
    pub(crate) refresh_rate_ms: Option<u64>,
    /// Warnings produced while importing the dashboard.
    pub(crate) diagnostics: Vec<ImportDiagnostic>,
}

/// A warning produced while importing a Grafana dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ImportDiagnostic {
    /// Stable diagnostic code.
    pub(crate) code: String,
    /// JSON-ish source path for the warning.
    pub(crate) path: String,
    /// Human-readable diagnostic message.
    pub(crate) message: String,
}

impl ImportDiagnostic {
    fn new(code: &str, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            path: path.into(),
            message: message.into(),
        }
    }
}

/// A Prometheus-backed Grafana template variable.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TemplateQueryVar {
    /// Variable name used in PromQL expressions.
    pub(crate) name: String,
    /// Prometheus variable query expression.
    pub(crate) query: String,
    /// Optional Grafana regex extractor.
    pub(crate) regex: Option<String>,
    /// JSON-ish source path for the variable query.
    pub(crate) query_path: String,
}

/// A single panel extracted from Grafana.
#[derive(Debug, Clone)]
pub(crate) struct QueryPanel {
    pub(crate) title: String,
    pub(crate) exprs: Vec<String>,
    pub(crate) expr_paths: Vec<String>,      // Parallel to exprs
    pub(crate) legends: Vec<Option<String>>, // Parallel to exprs
    pub(crate) query_modes: Vec<crate::app::QueryMode>, // Parallel to exprs
    pub(crate) grid: Option<GridPos>,
    pub(crate) panel_type: crate::app::PanelType,
    pub(crate) thresholds: Option<crate::app::Thresholds>,
    pub(crate) min: Option<f64>,
    pub(crate) max: Option<f64>,
    pub(crate) autogrid: Option<bool>,
    pub(crate) display: crate::ui::DisplayFormat,
    pub(crate) options: crate::app::PanelOptions,
}

/// Grid position extracted from Grafana.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GridPos {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
}

#[derive(Debug, Deserialize)]
struct RawDashboard {
    title: Option<String>,
    refresh: Option<serde_json::Value>,
    panels: Option<Vec<RawPanel>>,
    templating: Option<RawTemplating>,
}

#[derive(Debug, Deserialize)]
struct RawTemplating {
    list: Option<Vec<RawVar>>,
}

#[derive(Debug, Deserialize)]
struct RawVar {
    name: String,
    #[serde(rename = "type")]
    var_type: Option<String>,
    query: Option<RawVarQuery>,
    definition: Option<String>,
    regex: Option<String>,
    current: Option<RawVarCurrent>,
    /// The value to use when "All" is selected. Used to replace $__all in queries.
    #[serde(rename = "allValue")]
    all_value: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawVarQuery {
    String(String),
    Object { query: Option<String> },
}

#[derive(Debug, Deserialize)]
struct RawVarCurrent {
    text: Option<serde_json::Value>, // Can be string or array
    value: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RawPanel {
    #[serde(rename = "type")]
    panel_type: String,
    title: Option<String>,
    targets: Option<Vec<RawTarget>>,
    #[serde(rename = "gridPos")]
    grid_pos: Option<RawGridPos>,
    panels: Option<Vec<RawPanel>>, // nested rows
    #[serde(rename = "fieldConfig")]
    field_config: Option<RawFieldConfig>,
    options: Option<RawPanelOptions>,
}

#[derive(Debug, Deserialize)]
struct RawPanelOptions {
    #[serde(rename = "reduceOptions")]
    reduce_options: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RawFieldConfig {
    defaults: Option<RawFieldConfigDefaults>,
}

#[derive(Debug, Deserialize)]
struct RawFieldConfigDefaults {
    unit: Option<String>,
    decimals: Option<usize>,
    #[serde(rename = "noValue")]
    no_value: Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    thresholds: Option<RawThresholds>,
    custom: Option<RawCustom>,
    mappings: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RawCustom {
    #[serde(rename = "drawStyle")]
    draw_style: Option<String>,
    #[serde(rename = "showPoints")]
    show_points: Option<String>,
    #[serde(rename = "fillOpacity")]
    fill_opacity: Option<u16>,
    #[serde(rename = "axisPlacement")]
    axis_placement: Option<String>,
    #[serde(rename = "lineInterpolation")]
    line_interpolation: Option<String>,
    stacking: Option<RawStacking>,
    #[serde(rename = "axisGridShow")]
    axis_grid_show: Option<bool>,
    #[serde(rename = "thresholdsStyle")]
    thresholds_style: Option<RawThresholdsStyle>,
}

#[derive(Debug, Deserialize)]
struct RawThresholdsStyle {
    mode: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawStacking {
    mode: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawThresholds {
    mode: Option<String>,
    steps: Option<Vec<RawThresholdStep>>,
}

#[derive(Debug, Deserialize)]
struct RawThresholdStep {
    value: Option<f64>,
    color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawTarget {
    expr: Option<String>,
    #[serde(rename = "legendFormat")]
    legend_format: Option<String>,
    instant: Option<bool>,
    hide: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RawGridPos {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

pub(crate) fn load_grafana_dashboard(path: &std::path::Path) -> Result<DashboardImport> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("reading grafana dashboard: {}", path.display()))?;
    let raw: RawDashboard =
        serde_json::from_str(&data).with_context(|| "parsing grafana dashboard JSON")?;

    let mut vars = HashMap::new();
    let mut query_vars = Vec::new();
    if let Some(templating) = raw.templating {
        if let Some(list) = templating.list {
            for (var_idx, v) in list.into_iter().enumerate() {
                // Heuristic: prefer 'value' over 'text', handle arrays by taking first or joining?
                // Grafana 'current' value can be "All" or ["val1", "val2"].
                // For simple PromQL substitution, we usually want the raw value.
                // If it's "All", it might be $__all, which is tricky.
                // Let's try to get a string representation.
                let val = v
                    .current
                    .as_ref()
                    .and_then(|c| c.value.as_ref())
                    .or(v.current.as_ref().and_then(|c| c.text.as_ref()));

                if let Some(val) = val {
                    let mut s = match val {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Array(arr) => {
                            // If array, maybe join with pipe for regex? or just take first?
                            // For now, let's take the first string we find.
                            arr.iter()
                                .find_map(|x| x.as_str())
                                .unwrap_or("")
                                .to_string()
                        }
                        serde_json::Value::Number(n) => n.to_string(),
                        _ => String::new(),
                    };

                    // Handle $__all
                    if s == "$__all" {
                        // Use allValue if present, otherwise permissive regex
                        s = v.all_value.clone().unwrap_or_else(|| ".*".to_string());
                    }

                    if !s.is_empty() {
                        vars.insert(v.name.clone(), s);
                    }
                }

                if v.var_type.as_deref() == Some("query")
                    && !v.current_is_all()
                    && let Some(query) = v.query_string()
                {
                    query_vars.push(TemplateQueryVar {
                        name: v.name,
                        query,
                        regex: v.regex.filter(|regex| !regex.trim().is_empty()),
                        query_path: format!("templating.list[{var_idx}].query"),
                    });
                }
            }
        }
    }

    let mut out = DashboardImport {
        title: raw.title.unwrap_or_default(),
        refresh_rate_ms: raw.refresh.as_ref().and_then(parse_refresh_rate_ms),
        queries: vec![],
        vars,
        query_vars,
        skipped_panels: 0,
        diagnostics: vec![],
    };

    if let Some(panels) = raw.panels {
        collect_panels(&mut out, panels, "panels")?;
    }
    Ok(out)
}

impl RawVar {
    fn query_string(&self) -> Option<String> {
        let query = self
            .query
            .as_ref()
            .and_then(|query| match query {
                RawVarQuery::String(query) => Some(query.as_str()),
                RawVarQuery::Object { query } => query.as_deref(),
            })
            .or(self.definition.as_deref())?;

        let query = query.trim();
        (!query.is_empty()).then(|| query.to_string())
    }

    fn current_is_all(&self) -> bool {
        self.current.as_ref().is_some_and(|current| {
            value_is_all(current.value.as_ref()) || value_is_all(current.text.as_ref())
        })
    }
}

fn value_is_all(value: Option<&serde_json::Value>) -> bool {
    match value {
        Some(serde_json::Value::String(value)) => {
            value == "$__all" || value.eq_ignore_ascii_case("all")
        }
        Some(serde_json::Value::Array(values)) => {
            values.iter().any(|value| value_is_all(Some(value)))
        }
        _ => false,
    }
}

fn query_mode_for_target(
    instant: Option<bool>,
    panel_type: crate::app::PanelType,
) -> crate::app::QueryMode {
    match instant {
        Some(true) => crate::app::QueryMode::Instant,
        Some(false) => crate::app::QueryMode::Range,
        None => default_query_mode_for_panel(panel_type),
    }
}

fn default_query_mode_for_panel(panel_type: crate::app::PanelType) -> crate::app::QueryMode {
    match panel_type {
        crate::app::PanelType::Gauge
        | crate::app::PanelType::BarGauge
        | crate::app::PanelType::Table => crate::app::QueryMode::Instant,
        _ => crate::app::QueryMode::Range,
    }
}

fn graph_options_from_custom(custom: Option<&RawCustom>) -> crate::app::GraphOptions {
    let Some(custom) = custom else {
        return crate::app::GraphOptions::default();
    };

    crate::app::GraphOptions {
        draw_style: parse_graph_draw_style(custom.draw_style.as_deref()),
        show_points: parse_graph_point_mode(custom.show_points.as_deref()),
        fill_opacity: custom.fill_opacity.map(|value| value.min(100) as u8),
        axis_placement: parse_graph_axis_placement(custom.axis_placement.as_deref()),
        line_interpolation: custom
            .line_interpolation
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            .cloned(),
        stacking: parse_graph_stacking_mode(
            custom
                .stacking
                .as_ref()
                .and_then(|stacking| stacking.mode.as_deref()),
        ),
    }
}

fn parse_graph_draw_style(value: Option<&str>) -> crate::app::GraphDrawStyle {
    match value {
        Some("points") => crate::app::GraphDrawStyle::Points,
        Some("bars") => crate::app::GraphDrawStyle::Bars,
        _ => crate::app::GraphDrawStyle::Line,
    }
}

fn parse_graph_point_mode(value: Option<&str>) -> crate::app::GraphPointMode {
    match value {
        Some("always") => crate::app::GraphPointMode::Always,
        Some("never") => crate::app::GraphPointMode::Never,
        _ => crate::app::GraphPointMode::Auto,
    }
}

fn parse_graph_axis_placement(value: Option<&str>) -> crate::app::GraphAxisPlacement {
    match value {
        Some("hidden") => crate::app::GraphAxisPlacement::Hidden,
        _ => crate::app::GraphAxisPlacement::Visible,
    }
}

fn parse_graph_stacking_mode(value: Option<&str>) -> crate::app::GraphStackingMode {
    match value {
        Some("normal") => crate::app::GraphStackingMode::Normal,
        Some("percent") => crate::app::GraphStackingMode::Percent,
        _ => crate::app::GraphStackingMode::Off,
    }
}

fn collect_panels(out: &mut DashboardImport, panels: Vec<RawPanel>, path: &str) -> Result<()> {
    for (panel_idx, p) in panels.into_iter().enumerate() {
        let panel_path = format!("{path}[{panel_idx}]");
        if let Some(children) = p.panels {
            collect_panels(out, children, &format!("{panel_path}.panels"))?;
        }
        let kind = p.panel_type;
        let title = p.title.unwrap_or_default();

        let panel_type = match kind.as_str() {
            "graph" | "timeseries" => crate::app::PanelType::Graph,
            "stat" => crate::app::PanelType::Stat,
            "gauge" => crate::app::PanelType::Gauge,
            "bargauge" => crate::app::PanelType::BarGauge,
            "table" => crate::app::PanelType::Table,
            "heatmap" => crate::app::PanelType::Heatmap,
            _ => crate::app::PanelType::Unknown,
        };

        if panel_type != crate::app::PanelType::Unknown {
            let mut exprs = Vec::new();
            let mut expr_paths = Vec::new();
            let mut legends = Vec::new();
            let mut query_modes = Vec::new();

            for (target_idx, t) in p.targets.unwrap_or_default().into_iter().enumerate() {
                let target_path = format!("{panel_path}.targets[{target_idx}]");
                if t.hide == Some(true) {
                    continue;
                }
                if let Some(e) = t.expr {
                    exprs.push(e);
                    expr_paths.push(format!("{target_path}.expr"));
                    legends.push(t.legend_format);
                    query_modes.push(query_mode_for_target(t.instant, panel_type));
                }
            }

            let mut thresholds = None;
            let mut min = None;
            let mut max = None;
            let mut autogrid = None;
            let mut display = crate::ui::DisplayFormat::default();
            let mut graph_options = crate::app::GraphOptions::default();

            if let Some(options) = p.options {
                if options.reduce_options.is_some() {
                    out.diagnostics.push(ImportDiagnostic::new(
                        "ignored_field",
                        format!("{panel_path}.options.reduceOptions"),
                        "`options.reduceOptions` is not supported yet; Grafatui will use default value selection",
                    ));
                }
            }

            if let Some(fc) = p.field_config {
                if let Some(defaults) = fc.defaults {
                    if defaults.mappings.as_ref().is_some_and(non_empty_json_value) {
                        out.diagnostics.push(ImportDiagnostic::new(
                            "ignored_field",
                            format!("{panel_path}.fieldConfig.defaults.mappings"),
                            "`fieldConfig.defaults.mappings` is not supported yet; value mappings will be ignored",
                        ));
                    }

                    graph_options = graph_options_from_custom(defaults.custom.as_ref());
                    display = crate::ui::DisplayFormat {
                        unit: defaults.unit,
                        decimals: defaults.decimals,
                        no_value: defaults.no_value,
                    };
                    min = defaults.min;
                    max = defaults.max;
                    autogrid = defaults
                        .custom
                        .as_ref()
                        .and_then(|custom| custom.axis_grid_show);

                    if let Some(th) = defaults.thresholds {
                        let mode = match th.mode.as_deref() {
                            Some("percentage") => crate::app::ThresholdMode::Percentage,
                            _ => crate::app::ThresholdMode::Absolute,
                        };

                        let mut steps = Vec::new();
                        if let Some(raw_steps) = th.steps {
                            for s in raw_steps {
                                let color = s.color.unwrap_or_else(|| "green".to_string());
                                let parsed_color = crate::theme::parse_grafana_color(&color);
                                steps.push(crate::app::ThresholdStep {
                                    value: s.value,
                                    color: parsed_color,
                                });
                            }
                            steps.sort_by(|a, b| {
                                let a_val = a.value.unwrap_or(f64::NEG_INFINITY);
                                let b_val = b.value.unwrap_or(f64::NEG_INFINITY);
                                a_val
                                    .partial_cmp(&b_val)
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            });
                        }

                        if !steps.is_empty() {
                            let style = defaults
                                .custom
                                .as_ref()
                                .and_then(|c| c.thresholds_style.as_ref())
                                .and_then(|t| t.mode.clone())
                                .unwrap_or_else(|| "line".to_string());

                            thresholds = Some(crate::app::Thresholds {
                                mode,
                                steps,
                                style: Some(style),
                            });
                        }
                    }
                }
            }

            if !exprs.is_empty() {
                let gp = p.grid_pos.map(|g| GridPos {
                    x: g.x,
                    y: g.y,
                    w: g.w,
                    h: g.h,
                });
                let options = match panel_type {
                    crate::app::PanelType::Graph => crate::app::PanelOptions::Graph(graph_options),
                    _ => crate::app::PanelOptions::None,
                };
                out.queries.push(QueryPanel {
                    title,
                    exprs,
                    expr_paths,
                    legends,
                    query_modes,
                    grid: gp,
                    panel_type,
                    thresholds,
                    min,
                    max,
                    autogrid,
                    display,
                    options,
                });
            }
        } else if !kind.is_empty() && kind != "row" {
            // Count skipped panels (ignore rows)
            out.skipped_panels += 1;
            out.diagnostics.push(ImportDiagnostic::new(
                "skipped_panel",
                panel_path,
                format!("unsupported panel type `{kind}` skipped for panel `{title}`"),
            ));
        }
    }
    Ok(())
}

fn non_empty_json_value(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Array(values) => !values.is_empty(),
        serde_json::Value::Object(values) => !values.is_empty(),
        serde_json::Value::String(value) => !value.trim().is_empty(),
        serde_json::Value::Null => false,
        serde_json::Value::Bool(_) | serde_json::Value::Number(_) => true,
    }
}

pub(crate) fn variable_diagnostics(
    dashboard: &DashboardImport,
    vars: &HashMap<String, String>,
) -> Vec<ImportDiagnostic> {
    let mut known_vars: HashSet<String> = vars.keys().cloned().collect();
    known_vars.extend(dashboard.query_vars.iter().map(|var| var.name.clone()));

    let mut diagnostics = Vec::new();
    let mut seen = HashSet::new();
    for panel in &dashboard.queries {
        for (expr, path) in panel.exprs.iter().zip(panel.expr_paths.iter()) {
            collect_variable_diagnostics(expr, path, &known_vars, &mut diagnostics, &mut seen);
        }
    }
    for query_var in &dashboard.query_vars {
        collect_variable_diagnostics(
            &query_var.query,
            &query_var.query_path,
            &known_vars,
            &mut diagnostics,
            &mut seen,
        );
    }

    diagnostics
}

fn collect_variable_diagnostics(
    expr: &str,
    path: &str,
    known_vars: &HashSet<String>,
    diagnostics: &mut Vec<ImportDiagnostic>,
    seen: &mut HashSet<(String, String, String)>,
) {
    let chars: Vec<(usize, char)> = expr.char_indices().collect();
    let mut idx = 0;
    while idx < chars.len() {
        if chars[idx].1 != '$' {
            idx += 1;
            continue;
        }

        if idx + 1 >= chars.len() {
            idx += 1;
            continue;
        }

        if chars[idx + 1].1 == '{' {
            let start = chars[idx].0;
            let inner_start = chars[idx + 1].0 + 1;
            let mut end_idx = idx + 2;
            while end_idx < chars.len() && chars[end_idx].1 != '}' {
                end_idx += 1;
            }
            if end_idx >= chars.len() {
                idx += 1;
                continue;
            }

            let end = chars[end_idx].0;
            let token_end = end + 1;
            let inner = &expr[inner_start..end];
            let token = &expr[start..token_end];
            let (name, modifier) = inner.split_once(':').unwrap_or((inner, ""));
            if !modifier.is_empty() {
                push_variable_diagnostic(
                    diagnostics,
                    seen,
                    ImportDiagnostic::new(
                        "unsupported_variable_modifier",
                        path,
                        format!(
                            "unsupported Grafana variable modifier `{token}`; Grafatui expands only unmodified variables"
                        ),
                    ),
                );
            }
            if is_valid_variable_name(name)
                && !is_builtin_variable(name)
                && !known_vars.contains(name)
            {
                push_variable_diagnostic(
                    diagnostics,
                    seen,
                    ImportDiagnostic::new(
                        "unresolved_variable",
                        path,
                        format!(
                            "unresolved variable `{token}`; provide it with --var or dashboard templating"
                        ),
                    ),
                );
            }
            idx = end_idx + 1;
            continue;
        }

        let name_start = chars[idx + 1].0;
        let mut end_idx = idx + 1;
        while end_idx < chars.len() && is_variable_name_char(chars[end_idx].1) {
            end_idx += 1;
        }
        if end_idx == idx + 1 {
            idx += 1;
            continue;
        }

        let name_end = chars
            .get(end_idx)
            .map(|(byte_idx, _)| *byte_idx)
            .unwrap_or(expr.len());
        let name = &expr[name_start..name_end];
        if is_valid_variable_name(name) && !is_builtin_variable(name) && !known_vars.contains(name)
        {
            let token = &expr[chars[idx].0..name_end];
            push_variable_diagnostic(
                diagnostics,
                seen,
                ImportDiagnostic::new(
                    "unresolved_variable",
                    path,
                    format!(
                        "unresolved variable `{token}`; provide it with --var or dashboard templating"
                    ),
                ),
            );
        }
        idx = end_idx;
    }
}

fn push_variable_diagnostic(
    diagnostics: &mut Vec<ImportDiagnostic>,
    seen: &mut HashSet<(String, String, String)>,
    diagnostic: ImportDiagnostic,
) {
    let key = (
        diagnostic.code.clone(),
        diagnostic.path.clone(),
        diagnostic.message.clone(),
    );
    if seen.insert(key) {
        diagnostics.push(diagnostic);
    }
}

fn is_builtin_variable(name: &str) -> bool {
    matches!(
        name,
        "__interval"
            | "__interval_ms"
            | "__range"
            | "__range_s"
            | "__range_ms"
            | "__rate_interval"
            | "__rate_interval_ms"
    )
}

fn is_valid_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(is_variable_name_char)
}

fn is_variable_name_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn parse_refresh_rate_ms(value: &serde_json::Value) -> Option<u64> {
    let refresh = value.as_str()?.trim();
    if refresh.is_empty()
        || refresh.eq_ignore_ascii_case("false")
        || refresh.eq_ignore_ascii_case("off")
    {
        return None;
    }

    let duration = humantime::parse_duration(refresh).ok()?;
    u64::try_from(duration.as_millis()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dashboard_vars() {
        let json = r#"
        {
            "title": "Test Dash",
            "templating": {
                "list": [
                    {
                        "name": "job",
                        "current": { "text": "node-exporter", "value": "node-exporter" }
                    },
                    {
                        "name": "instance",
                        "current": { "text": "All", "value": ["server1", "server2"] }
                    }
                ]
            }
        }
        "#;

        let raw: RawDashboard = serde_json::from_str(json).unwrap();

        assert_eq!(raw.title.as_deref(), Some("Test Dash"));
        let list = raw.templating.unwrap().list.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "job");

        let v = &list[0];
        let val = v
            .current
            .as_ref()
            .and_then(|c| c.value.as_ref())
            .or(v.current.as_ref().and_then(|c| c.text.as_ref()));
        assert_eq!(val.unwrap().as_str(), Some("node-exporter"));
    }

    #[test]
    fn test_parse_axis_grid_show() {
        let json = r#"
        {
            "title": "Grid Test",
            "panels": [
                {
                    "type": "timeseries",
                    "title": "Grid Off",
                    "targets": [{ "expr": "up" }],
                    "fieldConfig": {
                        "defaults": {
                            "custom": {
                                "axisGridShow": false
                            }
                        }
                    }
                },
                {
                    "type": "timeseries",
                    "title": "Grid Default",
                    "targets": [{ "expr": "up" }]
                }
            ]
        }
        "#;
        let path = std::env::temp_dir().join("grafatui-axis-grid-test.json");
        std::fs::write(&path, json).unwrap();

        let dashboard = load_grafana_dashboard(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert_eq!(dashboard.queries[0].autogrid, Some(false));
        assert_eq!(dashboard.queries[1].autogrid, None);
    }

    #[test]
    fn test_parse_field_display_format() {
        let json = r#"
        {
            "title": "Display Format Test",
            "panels": [
                {
                    "type": "stat",
                    "title": "Memory",
                    "targets": [{ "expr": "process_resident_memory_bytes" }],
                    "fieldConfig": {
                        "defaults": {
                            "unit": "bytes",
                            "decimals": 1,
                            "noValue": "n/a"
                        }
                    }
                },
                {
                    "type": "stat",
                    "title": "Default",
                    "targets": [{ "expr": "up" }]
                }
            ]
        }
        "#;
        let path = std::env::temp_dir().join("grafatui-display-format-test.json");
        std::fs::write(&path, json).unwrap();

        let dashboard = load_grafana_dashboard(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert_eq!(dashboard.queries[0].display.unit.as_deref(), Some("bytes"));
        assert_eq!(dashboard.queries[0].display.decimals, Some(1));
        assert_eq!(
            dashboard.queries[0].display.no_value.as_deref(),
            Some("n/a")
        );
        assert_eq!(dashboard.queries[1].display.unit, None);
        assert_eq!(dashboard.queries[1].display.decimals, None);
        assert_eq!(dashboard.queries[1].display.no_value, None);
    }

    #[test]
    fn test_parse_target_instant_query_modes() {
        let json = r#"
        {
            "title": "Instant Mode Test",
            "panels": [
                {
                    "type": "timeseries",
                    "title": "Explicit Instant",
                    "targets": [
                        { "expr": "up", "instant": true },
                        { "expr": "rate(http_requests_total[5m])", "instant": false }
                    ]
                },
                {
                    "type": "gauge",
                    "title": "Gauge Default",
                    "targets": [{ "expr": "up" }]
                },
                {
                    "type": "bargauge",
                    "title": "Bar Gauge Default",
                    "targets": [{ "expr": "up" }]
                },
                {
                    "type": "table",
                    "title": "Table Default",
                    "targets": [{ "expr": "up" }]
                },
                {
                    "type": "stat",
                    "title": "Stat Default",
                    "targets": [{ "expr": "up" }]
                },
                {
                    "type": "gauge",
                    "title": "Gauge Range Override",
                    "targets": [{ "expr": "up", "instant": false }]
                }
            ]
        }
        "#;
        let path = std::env::temp_dir().join("grafatui-instant-mode-test.json");
        std::fs::write(&path, json).unwrap();

        let dashboard = load_grafana_dashboard(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert_eq!(
            dashboard.queries[0].query_modes,
            vec![crate::app::QueryMode::Instant, crate::app::QueryMode::Range]
        );
        assert_eq!(
            dashboard.queries[1].query_modes,
            vec![crate::app::QueryMode::Instant]
        );
        assert_eq!(
            dashboard.queries[2].query_modes,
            vec![crate::app::QueryMode::Instant]
        );
        assert_eq!(
            dashboard.queries[3].query_modes,
            vec![crate::app::QueryMode::Instant]
        );
        assert_eq!(
            dashboard.queries[4].query_modes,
            vec![crate::app::QueryMode::Range]
        );
        assert_eq!(
            dashboard.queries[5].query_modes,
            vec![crate::app::QueryMode::Range]
        );
    }

    #[test]
    fn test_parse_query_variables() {
        let json = r#"
        {
            "title": "Query Vars",
            "templating": {
                "list": [
                    {
                        "name": "instance",
                        "query": "label_values(up, instance)",
                        "type": "query",
                        "regex": "/(.+)/",
                        "includeAll": false,
                        "current": { "text": "node-1", "value": "node-1" }
                    },
                    {
                        "name": "model",
                        "query": { "query": "label_values(model_name)" },
                        "type": "query",
                        "current": { "text": "llama", "value": "llama" }
                    },
                    {
                        "name": "all_instance",
                        "query": "label_values(up, instance)",
                        "type": "query",
                        "allValue": ".*",
                        "current": { "text": "All", "value": "$__all" }
                    }
                ]
            }
        }
        "#;
        let path = std::env::temp_dir().join("grafatui-query-vars-test.json");
        std::fs::write(&path, json).unwrap();

        let dashboard = load_grafana_dashboard(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert_eq!(dashboard.query_vars.len(), 2);
        assert_eq!(dashboard.query_vars[0].query, "label_values(up, instance)");
        assert_eq!(dashboard.query_vars[0].regex.as_deref(), Some("/(.+)/"));
        assert_eq!(dashboard.query_vars[1].query, "label_values(model_name)");
        assert_eq!(dashboard.vars.get("all_instance"), Some(&".*".to_string()));
    }

    #[test]
    fn test_parse_dashboard_refresh_duration() {
        let json = r#"
        {
            "title": "Refresh Dash",
            "refresh": "5s",
            "panels": [
                {
                    "type": "timeseries",
                    "title": "Up",
                    "targets": [{ "expr": "up" }]
                }
            ]
        }
        "#;
        let path = std::env::temp_dir().join("grafatui-refresh-test.json");
        std::fs::write(&path, json).unwrap();

        let dashboard = load_grafana_dashboard(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert_eq!(dashboard.refresh_rate_ms, Some(5000));
    }

    #[test]
    fn test_import_timeseries_graph_options() {
        let json = r#"{
            "title": "Graph options",
            "panels": [{
                "type": "timeseries",
                "title": "Area points",
                "targets": [{ "expr": "up" }],
                "fieldConfig": {
                    "defaults": {
                        "custom": {
                            "drawStyle": "line",
                            "showPoints": "always",
                            "fillOpacity": 20,
                            "axisPlacement": "hidden",
                            "lineInterpolation": "smooth",
                            "stacking": { "mode": "normal" }
                        }
                    }
                }
            }]
        }"#;

        let raw: RawDashboard = serde_json::from_str(json).unwrap();
        let mut out = DashboardImport {
            title: raw.title.unwrap_or_default(),
            refresh_rate_ms: None,
            queries: vec![],
            vars: HashMap::new(),
            query_vars: vec![],
            skipped_panels: 0,
            diagnostics: vec![],
        };

        collect_panels(&mut out, raw.panels.unwrap(), "panels").unwrap();

        let options = match &out.queries[0].options {
            crate::app::PanelOptions::Graph(options) => options,
            other => panic!("expected graph options, got {other:?}"),
        };
        assert_eq!(options.draw_style, crate::app::GraphDrawStyle::Line);
        assert_eq!(options.show_points, crate::app::GraphPointMode::Always);
        assert_eq!(options.fill_opacity, Some(20));
        assert_eq!(
            options.axis_placement,
            crate::app::GraphAxisPlacement::Hidden
        );
        assert_eq!(options.line_interpolation.as_deref(), Some("smooth"));
        assert_eq!(options.stacking, crate::app::GraphStackingMode::Normal);
    }

    #[test]
    fn test_import_graph_options_fallbacks_and_non_graph_none() {
        let json = r#"{
            "title": "Fallbacks",
            "panels": [
                {
                    "type": "timeseries",
                    "title": "Unknown values",
                    "targets": [{ "expr": "up" }],
                    "fieldConfig": {
                        "defaults": {
                            "custom": {
                                "drawStyle": "candles",
                                "showPoints": "sometimes",
                                "fillOpacity": 999,
                                "axisPlacement": "right",
                                "stacking": { "mode": "percent" }
                            }
                        }
                    }
                },
                {
                    "type": "stat",
                    "title": "Stat",
                    "targets": [{ "expr": "up" }]
                }
            ]
        }"#;

        let raw: RawDashboard = serde_json::from_str(json).unwrap();
        let mut out = DashboardImport {
            title: raw.title.unwrap_or_default(),
            refresh_rate_ms: None,
            queries: vec![],
            vars: HashMap::new(),
            query_vars: vec![],
            skipped_panels: 0,
            diagnostics: vec![],
        };

        collect_panels(&mut out, raw.panels.unwrap(), "panels").unwrap();

        let graph_options = match &out.queries[0].options {
            crate::app::PanelOptions::Graph(options) => options,
            other => panic!("expected graph options, got {other:?}"),
        };
        assert_eq!(graph_options.draw_style, crate::app::GraphDrawStyle::Line);
        assert_eq!(graph_options.show_points, crate::app::GraphPointMode::Auto);
        assert_eq!(graph_options.fill_opacity, Some(100));
        assert_eq!(
            graph_options.axis_placement,
            crate::app::GraphAxisPlacement::Visible
        );
        assert_eq!(
            graph_options.stacking,
            crate::app::GraphStackingMode::Percent
        );
        assert_eq!(out.queries[1].options, crate::app::PanelOptions::None);
    }

    #[test]
    fn test_import_diagnostics_report_skipped_panel_type() {
        let json = r#"{
            "title": "Skipped",
            "panels": [
                { "type": "text", "title": "Notes" }
            ]
        }"#;
        let path = std::env::temp_dir().join("grafatui-skipped-panel-diagnostics.json");
        std::fs::write(&path, json).unwrap();

        let dashboard = load_grafana_dashboard(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert_eq!(dashboard.skipped_panels, 1);
        assert_eq!(dashboard.diagnostics.len(), 1);
        assert_eq!(dashboard.diagnostics[0].code, "skipped_panel");
        assert_eq!(dashboard.diagnostics[0].path, "panels[0]");
        assert!(
            dashboard.diagnostics[0]
                .message
                .contains("unsupported panel type `text`")
        );
        assert!(dashboard.diagnostics[0].message.contains("Notes"));
    }

    #[test]
    fn test_import_diagnostics_report_ignored_high_impact_fields() {
        let json = r#"{
            "title": "Ignored Fields",
            "panels": [
                {
                    "type": "stat",
                    "title": "CPU",
                    "targets": [
                        { "expr": "up" }
                    ],
                    "fieldConfig": {
                        "defaults": {
                            "mappings": [
                                { "type": "value", "options": { "0": { "text": "Down" } } }
                            ]
                        }
                    },
                    "options": {
                        "reduceOptions": {
                            "calcs": ["mean"]
                        }
                    }
                }
            ]
        }"#;
        let path = std::env::temp_dir().join("grafatui-ignored-fields-diagnostics.json");
        std::fs::write(&path, json).unwrap();

        let dashboard = load_grafana_dashboard(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        let diagnostics: Vec<_> = dashboard
            .diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.code.as_str(), diagnostic.path.as_str()))
            .collect();

        assert!(
            diagnostics.contains(&("ignored_field", "panels[0].fieldConfig.defaults.mappings"))
        );
        assert!(diagnostics.contains(&("ignored_field", "panels[0].options.reduceOptions")));
    }

    #[test]
    fn test_hidden_targets_are_not_imported_or_warned() {
        let json = r#"{
            "title": "Hidden Targets",
            "panels": [
                {
                    "type": "timeseries",
                    "title": "CPU",
                    "targets": [
                        { "expr": "helper_query", "hide": true },
                        { "expr": "visible_query" }
                    ]
                }
            ]
        }"#;
        let path = std::env::temp_dir().join("grafatui-hidden-targets-test.json");
        std::fs::write(&path, json).unwrap();

        let dashboard = load_grafana_dashboard(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert_eq!(dashboard.queries.len(), 1);
        assert_eq!(dashboard.queries[0].exprs, vec!["visible_query"]);
        assert_eq!(
            dashboard.queries[0].expr_paths,
            vec!["panels[0].targets[1].expr"]
        );
        assert!(
            dashboard
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.path != "panels[0].targets[0].hide")
        );
    }

    #[test]
    fn test_import_diagnostics_preserve_nested_row_paths() {
        let json = r#"{
            "title": "Rows",
            "panels": [
                {
                    "type": "row",
                    "title": "Group",
                    "panels": [
                        { "type": "piechart", "title": "Pie" }
                    ]
                }
            ]
        }"#;
        let path = std::env::temp_dir().join("grafatui-nested-row-diagnostics.json");
        std::fs::write(&path, json).unwrap();

        let dashboard = load_grafana_dashboard(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert_eq!(dashboard.diagnostics[0].code, "skipped_panel");
        assert_eq!(dashboard.diagnostics[0].path, "panels[0].panels[0]");
    }

    #[test]
    fn test_variable_diagnostics_report_modifiers_and_unresolved_variables() {
        let json = r#"{
            "title": "Variables",
            "templating": {
                "list": [
                    { "name": "job", "current": { "text": "node", "value": "node" } },
                    { "name": "instance", "current": { "text": "server", "value": "server" } },
                    {
                        "name": "query_var",
                        "type": "query",
                        "query": "label_values(up{job=\"$job\"}, instance)",
                        "current": { "text": "server", "value": "server" }
                    }
                ]
            },
            "panels": [
                {
                    "type": "timeseries",
                    "title": "CPU",
                    "targets": [
                        { "expr": "up{job=\"$job\", instance=\"${instance}\"}" },
                        { "expr": "up{job=~\"${job:regex}\", cluster=\"$cluster\", interval=\"$__interval\", range=\"$__range_s\"}" }
                    ]
                }
            ]
        }"#;
        let path = std::env::temp_dir().join("grafatui-variable-diagnostics.json");
        std::fs::write(&path, json).unwrap();

        let dashboard = load_grafana_dashboard(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        let diagnostics = variable_diagnostics(&dashboard, &dashboard.vars);
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "unsupported_variable_modifier"
                && diagnostic.path == "panels[0].targets[1].expr"
                && diagnostic.message.contains("${job:regex}")
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "unresolved_variable"
                && diagnostic.path == "panels[0].targets[1].expr"
                && diagnostic.message.contains("$cluster")
        }));
    }
}
