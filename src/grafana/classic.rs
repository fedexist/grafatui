use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

use super::model;

#[derive(Debug, Deserialize)]
struct RawDashboard {
    title: Option<String>,
    refresh: Option<Value>,
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
    text: Option<Value>,
    value: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct RawPanel {
    #[serde(rename = "type")]
    panel_type: String,
    title: Option<String>,
    targets: Option<Vec<RawTarget>>,
    #[serde(rename = "gridPos")]
    grid_pos: Option<RawGridPos>,
    panels: Option<Vec<RawPanel>>,
    #[serde(rename = "fieldConfig")]
    field_config: Option<RawFieldConfig>,
    options: Option<RawPanelOptions>,
}

#[derive(Debug, Deserialize)]
struct RawPanelOptions {
    #[serde(rename = "reduceOptions")]
    reduce_options: Option<Value>,
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
    mappings: Option<Value>,
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

pub(super) fn adapt(value: Value) -> Result<model::Dashboard> {
    let raw: RawDashboard =
        serde_json::from_value(value).context("parsing Grafana Classic dashboard JSON")?;
    let mut dashboard = model::Dashboard {
        title: raw.title.unwrap_or_default(),
        refresh: raw
            .refresh
            .and_then(|value| value.as_str().map(str::to_owned)),
        ..model::Dashboard::default()
    };
    dashboard.variables = raw
        .templating
        .and_then(|templating| templating.list)
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(index, variable)| variable.normalize(index))
        .collect();
    normalize_panels(
        raw.panels.unwrap_or_default(),
        "panels",
        &mut dashboard.layout,
    );
    Ok(dashboard)
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

    fn normalize(self, index: usize) -> model::Variable {
        let query = self.query_string();
        model::Variable {
            name: self.name,
            kind: self.var_type,
            current: self.current.map(|current| model::VariableCurrent {
                text: current.text,
                value: current.value,
            }),
            query_path: query
                .is_some()
                .then(|| format!("templating.list[{index}].query")),
            query,
            regex: self.regex,
            all_value: self.all_value,
            source_path: format!("templating.list[{index}]"),
        }
    }
}

fn normalize_panels(panels: Vec<RawPanel>, path: &str, out: &mut Vec<model::LayoutNode>) {
    for (index, panel) in panels.into_iter().enumerate() {
        let source_path = format!("{path}[{index}]");
        if let Some(children) = panel.panels {
            normalize_panels(children, &format!("{source_path}.panels"), out);
        }

        let field_defaults = panel.field_config.and_then(|config| {
            config.defaults.map(|defaults| model::FieldDefaults {
                unit: defaults.unit,
                decimals: defaults.decimals,
                no_value: defaults.no_value,
                min: defaults.min,
                max: defaults.max,
                thresholds: defaults.thresholds.map(|thresholds| model::Thresholds {
                    mode: thresholds.mode,
                    steps: thresholds
                        .steps
                        .unwrap_or_default()
                        .into_iter()
                        .map(|step| model::ThresholdStep {
                            value: step.value,
                            color: step.color,
                        })
                        .collect(),
                }),
                custom: defaults.custom.map(|custom| model::GraphCustom {
                    draw_style: custom.draw_style,
                    show_points: custom.show_points,
                    fill_opacity: custom.fill_opacity,
                    axis_placement: custom.axis_placement,
                    line_interpolation: custom.line_interpolation,
                    stacking_mode: custom.stacking.and_then(|stacking| stacking.mode),
                    axis_grid_show: custom.axis_grid_show,
                    thresholds_style_mode: custom.thresholds_style.and_then(|style| style.mode),
                }),
                mappings_path: defaults
                    .mappings
                    .as_ref()
                    .is_some_and(non_empty_json_value)
                    .then(|| format!("{source_path}.fieldConfig.defaults.mappings")),
            })
        });
        out.push(model::LayoutNode::Panel(model::Panel {
            kind: panel.panel_type,
            title: panel.title.unwrap_or_default(),
            source_path: source_path.clone(),
            targets: panel
                .targets
                .unwrap_or_default()
                .into_iter()
                .enumerate()
                .map(|(index, target)| model::Target {
                    expr: target.expr,
                    expr_path: format!("{source_path}.targets[{index}].expr"),
                    legend_format: target.legend_format,
                    instant: target.instant,
                    hidden: target.hide == Some(true),
                })
                .collect(),
            count_as_skipped_if_empty: false,
            grid: panel.grid_pos.map(|grid| model::GridPos {
                x: grid.x,
                y: grid.y,
                w: grid.w,
                h: grid.h,
            }),
            field_defaults,
            reduce_options_path: panel
                .options
                .and_then(|options| options.reduce_options)
                .is_some()
                .then(|| format!("{source_path}.options.reduceOptions")),
            transformations_path: None,
        }));
    }
}

fn non_empty_json_value(value: &Value) -> bool {
    match value {
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
        Value::String(value) => !value.trim().is_empty(),
        Value::Null => false,
        Value::Bool(_) | Value::Number(_) => true,
    }
}
