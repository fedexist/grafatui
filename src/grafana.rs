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

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

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
}

/// A single panel extracted from Grafana.
#[derive(Debug, Clone)]
pub(crate) struct QueryPanel {
    pub(crate) title: String,
    pub(crate) exprs: Vec<String>,
    pub(crate) legends: Vec<Option<String>>, // Parallel to exprs
    pub(crate) grid: Option<GridPos>,
    pub(crate) panel_type: crate::app::PanelType,
    pub(crate) thresholds: Option<crate::app::Thresholds>,
    pub(crate) min: Option<f64>,
    pub(crate) max: Option<f64>,
    pub(crate) autogrid: Option<bool>,
    pub(crate) display: crate::ui::DisplayFormat,
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
}

#[derive(Debug, Deserialize)]
struct RawCustom {
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
            for v in list {
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
    };

    if let Some(panels) = raw.panels {
        collect_panels(&mut out, panels)?;
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

fn collect_panels(out: &mut DashboardImport, panels: Vec<RawPanel>) -> Result<()> {
    for p in panels.into_iter() {
        if let Some(children) = p.panels {
            collect_panels(out, children)?;
        }
        let kind = p.panel_type;

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
            let mut legends = Vec::new();

            for t in p.targets.unwrap_or_default() {
                if let Some(e) = t.expr {
                    exprs.push(e);
                    legends.push(t.legend_format);
                }
            }

            let mut thresholds = None;
            let mut min = None;
            let mut max = None;
            let mut autogrid = None;
            let mut display = crate::ui::DisplayFormat::default();

            if let Some(fc) = p.field_config {
                if let Some(defaults) = fc.defaults {
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
                                .and_then(|c| c.thresholds_style)
                                .and_then(|t| t.mode)
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
                out.queries.push(QueryPanel {
                    title: p.title.unwrap_or_default(),
                    exprs,
                    legends,
                    grid: gp,
                    panel_type,
                    thresholds,
                    min,
                    max,
                    autogrid,
                    display,
                });
            }
        } else if !kind.is_empty() && kind != "row" {
            // Count skipped panels (ignore rows)
            out.skipped_panels += 1;
        }
    }
    Ok(())
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
}
