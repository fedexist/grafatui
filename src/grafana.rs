use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Default)]
pub struct DashboardImport {
    pub title: String,
    pub queries: Vec<QueryPanel>,
}

#[derive(Debug, Clone)]
pub struct QueryPanel {
    pub title: String,
    pub exprs: Vec<String>,
    pub grid: Option<GridPos>,
}

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
    let mut out = DashboardImport {
        title: raw.title.unwrap_or_default(),
        queries: vec![],
    };
    if let Some(panels) = raw.panels {
        collect_panels(&mut out.queries, panels)?;
    }
    Ok(out)
}

fn collect_panels(out: &mut Vec<QueryPanel>, panels: Vec<RawPanel>) -> Result<()> {
    for p in panels.into_iter() {
        if let Some(children) = p.panels {
            collect_panels(out, children)?;
        }
        let kind = p.panel_type.unwrap_or_default();
        if kind == "graph" || kind == "timeseries" || kind == "stat" {
            let exprs: Vec<String> = p
                .targets
                .unwrap_or_default()
                .into_iter()
                .filter_map(|t| t.expr)
                .collect();
            if !exprs.is_empty() {
                let gp = p.grid_pos.map(|g| GridPos {
                    x: g.x,
                    y: g.y,
                    w: g.w,
                    h: g.h,
                });
                out.push(QueryPanel {
                    title: p.title.unwrap_or_default(),
                    exprs,
                    grid: gp,
                });
            }
        }
    }
    Ok(())
}
