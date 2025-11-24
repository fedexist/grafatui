use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

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
    panel_type: Option<String>,
    title: Option<String>,
    targets: Option<Vec<RawTarget>>,
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

pub fn load_grafana_dashboard(path: &std::path::Path) -> Result<DashboardImport> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("reading grafana dashboard: {}", path.display()))?;
    let raw: RawDashboard =
        serde_json::from_str(&data).with_context(|| "parsing grafana dashboard JSON")?;

    let mut vars = HashMap::new();
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
                    let s = match val {
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
                    if !s.is_empty() && s != "All" {
                        vars.insert(v.name, s);
                    }
                }
            }
        }
    }

    let mut out = DashboardImport {
        title: raw.title.unwrap_or_default(),
        queries: vec![],
        vars,
        skipped_panels: 0,
    };

    if let Some(panels) = raw.panels {
        collect_panels(&mut out, panels)?;
    }
    Ok(out)
}

fn collect_panels(out: &mut DashboardImport, panels: Vec<RawPanel>) -> Result<()> {
    for p in panels.into_iter() {
        if let Some(children) = p.panels {
            collect_panels(out, children)?;
        }
        let kind = p.panel_type.unwrap_or_default();

        let panel_type = match kind.as_str() {
            "graph" | "timeseries" => crate::app::PanelType::Graph,
            "stat" => crate::app::PanelType::Stat,
            "gauge" => crate::app::PanelType::Gauge,
            "bargauge" => crate::app::PanelType::BarGauge,
            "table" => crate::app::PanelType::Table,
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
            }
        } else if !kind.is_empty() && kind != "row" {
            // Count skipped panels (ignore rows)
            out.skipped_panels += 1;
        }
    }
    Ok(())
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
}
