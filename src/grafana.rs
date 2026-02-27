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

/// A skipped / unsupported panel encountered during import or validation.
#[derive(Debug, Clone)]
pub struct SkippedPanel {
    pub panel_type: String,
    pub title: String,
    pub reason: String,
}

/// Result of importing a Grafana dashboard.
#[derive(Debug, Clone, Default)]
pub struct DashboardImport {
    /// Dashboard title.
    pub title: String,
    /// List of panels extracted.
    pub queries: Vec<QueryPanel>,
    /// Variables extracted from `templating.list`.
    pub vars: HashMap<String, String>,
    /// Number of panels that were skipped (unsupported types).
    pub skipped_panels: usize,
    /// Details about skipped panels (unsupported panel types, or panels without queries).
    pub skipped: Vec<SkippedPanel>,
}

/// A single panel extracted from Grafana.
#[derive(Debug, Clone)]
pub struct QueryPanel {
    pub title: String,
    pub exprs: Vec<String>,
    pub legends: Vec<Option<String>>, // Parallel to exprs
    pub grid: Option<GridPos>,
    pub panel_type: crate::app::PanelType,
}

/// Grid position extracted from Grafana.
#[derive(Debug, Clone, Copy)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Deserialize)]
struct RawDashboard {
    title: Option<String>,
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
    current: Option<RawVarCurrent>,
    /// The value to use when "All" is selected. Used to replace $__all in queries.
    #[serde(rename = "allValue")]
    all_value: Option<String>,
    // We could parse 'query' or 'type' if needed, but for now we just want defaults
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

/// A human-friendly validation report for a Grafana dashboard JSON file.
#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    pub title: String,
    pub vars: HashMap<String, String>,
    /// Number of non-row panels encountered in the JSON.
    pub total_panels: usize,
    /// Number of panels that can be imported and rendered (supported type + at least one PromQL expr).
    pub supported_panels: usize,
    /// Panels that cannot be rendered or imported (unsupported type or missing query targets).
    pub skipped: Vec<SkippedPanel>,
}

impl ValidationReport {
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str("Grafana dashboard validation\n");
        out.push_str("===========================\n");
        out.push_str(&format!("Title: {}\n", if self.title.is_empty() { "-" } else { &self.title }));
        out.push_str(&format!(
            "Panels: total={} supported={} skipped={}\n",
            self.total_panels,
            self.supported_panels,
            self.skipped.len()
        ));
        out.push_str(&format!("Variables: {}\n", self.vars.len()));
        if !self.vars.is_empty() {
            let mut keys: Vec<_> = self.vars.keys().cloned().collect();
            keys.sort();
            for k in keys {
                let v = self.vars.get(&k).map(String::as_str).unwrap_or("-");
                out.push_str(&format!("  - {}={}\n", k, v));
            }
        }
        if !self.skipped.is_empty() {
            out.push('\n');
            out.push_str("Skipped panels:\n");
            for p in &self.skipped {
                let title = if p.title.is_empty() { "-" } else { &p.title };
                out.push_str(&format!(
                    "  - type={} title={} reason={}\n",
                    p.panel_type, title, p.reason
                ));
            }
        }
        out
    }
}

pub fn load_grafana_dashboard(path: &std::path::Path) -> Result<DashboardImport> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("reading grafana dashboard: {}", path.display()))?;
    let raw: RawDashboard = parse_raw_dashboard(&data)?;
    let vars = extract_vars(&raw);

    let mut out = DashboardImport {
        title: raw.title.unwrap_or_default(),
        queries: vec![],
        vars,
        skipped_panels: 0,
        skipped: vec![],
    };

    if let Some(panels) = raw.panels {
        collect_panels(&mut out, panels)?;
    }
    Ok(out)
}

pub fn validate_grafana_dashboard(path: &std::path::Path) -> Result<ValidationReport> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("reading grafana dashboard: {}", path.display()))?;
    let raw: RawDashboard = parse_raw_dashboard(&data)?;

    let mut report = ValidationReport {
        title: raw.title.clone().unwrap_or_default(),
        vars: extract_vars(&raw),
        total_panels: 0,
        supported_panels: 0,
        skipped: Vec::new(),
    };

    if let Some(panels) = raw.panels {
        collect_validation(&mut report, panels);
    }

    Ok(report)
}

fn parse_raw_dashboard(data: &str) -> Result<RawDashboard> {
    serde_json::from_str(data).with_context(|| "parsing grafana dashboard JSON")
}

fn extract_vars(raw: &RawDashboard) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    if let Some(templating) = raw.templating.as_ref() {
        if let Some(list) = templating.list.as_ref() {
            for v in list {
                // Heuristic: prefer 'value' over 'text', handle arrays by taking first string.
                let val = v
                    .current
                    .as_ref()
                    .and_then(|c| c.value.as_ref())
                    .or(v.current.as_ref().and_then(|c| c.text.as_ref()));

                if let Some(val) = val {
                    let mut s = match val {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Array(arr) => arr
                            .iter()
                            .find_map(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        serde_json::Value::Number(n) => n.to_string(),
                        _ => String::new(),
                    };

                    if s == "$__all" {
                        s = v.all_value.clone().unwrap_or_else(|| ".*".to_string());
                    }

                    if !s.is_empty() {
                        vars.insert(v.name.clone(), s);
                    }
                }
            }
        }
    }

    vars
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
                });
            } else if kind != "row" {
                out.skipped_panels += 1;
                out.skipped.push(SkippedPanel {
                    panel_type: kind,
                    title: p.title.unwrap_or_default(),
                    reason: "no PromQL targets found".to_string(),
                });
            }
        } else if !kind.is_empty() && kind != "row" {
            // Count skipped panels (ignore rows)
            out.skipped_panels += 1;
            out.skipped.push(SkippedPanel {
                panel_type: kind,
                title: p.title.unwrap_or_default(),
                reason: "unsupported panel type".to_string(),
            });
        }
    }
    Ok(())
}

fn collect_validation(report: &mut ValidationReport, panels: Vec<RawPanel>) {
    for p in panels.into_iter() {
        if let Some(children) = p.panels {
            collect_validation(report, children);
        }

        let kind = p.panel_type;
        if kind.is_empty() || kind == "row" {
            continue;
        }

        report.total_panels += 1;

        let is_supported_type = matches!(
            kind.as_str(),
            "graph" | "timeseries" | "stat" | "gauge" | "bargauge" | "table" | "heatmap"
        );

        if !is_supported_type {
            report.skipped.push(SkippedPanel {
                panel_type: kind,
                title: p.title.unwrap_or_default(),
                reason: "unsupported panel type".to_string(),
            });
            continue;
        }

        let has_expr = p
            .targets
            .unwrap_or_default()
            .into_iter()
            .any(|t| t.expr.as_ref().is_some_and(|e| !e.trim().is_empty()));

        if has_expr {
            report.supported_panels += 1;
        } else {
            report.skipped.push(SkippedPanel {
                panel_type: kind,
                title: p.title.unwrap_or_default(),
                reason: "no PromQL targets found".to_string(),
            });
        }
    }
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
    fn test_validation_counts_unsupported_and_empty_targets() {
        let json = r#"
        {
            "title": "Dash",
            "panels": [
                { "type": "timeseries", "title": "OK", "targets": [ { "expr": "up" } ] },
                { "type": "timeseries", "title": "Empty", "targets": [] },
                { "type": "custom", "title": "Unsupported", "targets": [ { "expr": "up" } ] },
                { "type": "row", "title": "Row", "panels": [
                    { "type": "graph", "title": "Nested OK", "targets": [ { "expr": "up" } ] }
                ]}
            ]
        }
        "#;

        let raw: RawDashboard = serde_json::from_str(json).unwrap();
        let mut report = ValidationReport {
            title: raw.title.clone().unwrap_or_default(),
            vars: extract_vars(&raw),
            total_panels: 0,
            supported_panels: 0,
            skipped: Vec::new(),
        };

        collect_validation(&mut report, raw.panels.unwrap());

        assert_eq!(report.total_panels, 4); // excludes row, includes nested
        assert_eq!(report.supported_panels, 2);
        assert_eq!(report.skipped.len(), 2);
    }
}
