use anyhow::{Context, Result, anyhow, ensure};
use serde::Deserialize;
use serde_json::Value;

use super::model;

pub(super) const V2_API_VERSION: &str = "dashboard.grafana.app/v2";

type JsonObject = serde_json::Map<String, Value>;

struct ResolvedGridItem {
    element_name: String,
    position: model::GridPos,
}

pub(super) fn adapt(value: Value) -> Result<model::Dashboard> {
    let root = value
        .as_object()
        .ok_or_else(|| anyhow!("invalid Grafana dashboard at $: expected an object"))?;
    require_string_from(root, "kind", "kind")?
        .eq("Dashboard")
        .then_some(())
        .ok_or_else(|| anyhow!("invalid Grafana V2 resource kind at kind: expected `Dashboard`"))?;
    let spec = require_object_from(root, "spec", "spec")?;
    let title = require_string_from(spec, "title", "spec.title")?.to_string();
    let elements = require_object_from(spec, "elements", "spec.elements")?;
    let layout = require_object_from(spec, "layout", "spec.layout")?;
    let layout_kind = require_string_from(layout, "kind", "spec.layout.kind")?;
    ensure!(
        layout_kind == "GridLayout",
        "unsupported Grafana V2 layout `{layout_kind}` at spec.layout.kind; this release supports `GridLayout` only"
    );
    let layout_spec = require_object_from(layout, "spec", "spec.layout.spec")?;
    let items = require_array_from(layout_spec, "items", "spec.layout.spec.items")?;

    let mut dashboard = model::Dashboard {
        title,
        ..model::Dashboard::default()
    };
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("spec.layout.spec.items[{index}]");
        let grid = parse_grid_item(item, &item_path)?;
        let element_path = format!("spec.elements[{:?}]", grid.element_name);
        let element = elements.get(&grid.element_name).ok_or_else(|| {
            anyhow!(
                "unresolved Grafana V2 element reference `{}` at {item_path}.spec.element.name",
                grid.element_name
            )
        })?;
        if let Some(panel) = parse_panel(
            element,
            &element_path,
            grid.position,
            &mut dashboard.diagnostics,
        )? {
            dashboard.panels.push(panel);
        }
    }
    Ok(dashboard)
}

fn require_object_from<'a>(
    object: &'a JsonObject,
    key: &str,
    path: &str,
) -> Result<&'a JsonObject> {
    object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("invalid Grafana V2 resource at {path}: expected an object"))
}

fn require_array_from<'a>(object: &'a JsonObject, key: &str, path: &str) -> Result<&'a Vec<Value>> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("invalid Grafana V2 resource at {path}: expected an array"))
}

fn require_string_from<'a>(object: &'a JsonObject, key: &str, path: &str) -> Result<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("invalid Grafana V2 resource at {path}: expected a string"))
}

fn require_i32_from(object: &JsonObject, key: &str, path: &str) -> Result<i32> {
    let value = object.get(key).and_then(Value::as_i64).ok_or_else(|| {
        anyhow!("invalid Grafana V2 grid coordinate at {path}: expected an integer")
    })?;
    i32::try_from(value)
        .map_err(|_| anyhow!("invalid Grafana V2 grid coordinate at {path}: expected an i32"))
}

fn parse_grid_item(value: &Value, path: &str) -> Result<ResolvedGridItem> {
    let item = value
        .as_object()
        .ok_or_else(|| anyhow!("invalid Grafana V2 grid item at {path}: expected an object"))?;
    let kind_path = format!("{path}.kind");
    let kind = require_string_from(item, "kind", &kind_path)?;
    ensure!(
        kind == "GridLayoutItem",
        "invalid Grafana V2 grid item kind `{kind}` at {kind_path}: expected `GridLayoutItem`"
    );
    let spec_path = format!("{path}.spec");
    let spec = require_object_from(item, "spec", &spec_path)?;
    let repeat_path = format!("{spec_path}.repeat");
    ensure!(
        !spec.contains_key("repeat"),
        "unsupported Grafana V2 repeated grid item at {repeat_path}"
    );
    let x = require_i32_from(spec, "x", &format!("{spec_path}.x"))?;
    let y = require_i32_from(spec, "y", &format!("{spec_path}.y"))?;
    let w = require_i32_from(spec, "width", &format!("{spec_path}.width"))?;
    let h = require_i32_from(spec, "height", &format!("{spec_path}.height"))?;
    let element_path = format!("{spec_path}.element");
    let element = require_object_from(spec, "element", &element_path)?;
    let element_kind_path = format!("{element_path}.kind");
    let element_kind = require_string_from(element, "kind", &element_kind_path)?;
    ensure!(
        element_kind == "ElementReference",
        "invalid Grafana V2 grid element kind `{element_kind}` at {element_kind_path}: expected `ElementReference`"
    );
    let element_name_path = format!("{element_path}.name");
    let element_name = require_string_from(element, "name", &element_name_path)?.to_string();

    Ok(ResolvedGridItem {
        element_name,
        position: model::GridPos { x, y, w, h },
    })
}

fn parse_panel(
    value: &Value,
    path: &str,
    grid: model::GridPos,
    _diagnostics: &mut Vec<super::ImportDiagnostic>,
) -> Result<Option<model::Panel>> {
    let raw: RawPanelElement = serde_json::from_value(value.clone())
        .with_context(|| format!("parsing Grafana V2 panel at {path}"))?;
    if raw.kind != "Panel" {
        return Ok(None);
    }

    let panel_path = format!("{path}.spec");
    let data_path = format!("{panel_path}.data.spec.queries");
    let targets = raw
        .spec
        .data
        .spec
        .queries
        .into_iter()
        .enumerate()
        .filter_map(|(index, query)| {
            (query.spec.query.group == "prometheus").then(|| model::Target {
                expr: query.spec.query.spec.expr,
                expr_path: format!("{data_path}[{index}].spec.query.spec.expr"),
                legend_format: query.spec.query.spec.legend_format,
                instant: query.spec.query.spec.instant,
                hidden: query.spec.hidden,
            })
        })
        .collect();
    let viz_spec = raw.spec.viz_config.spec;
    let defaults = viz_spec.field_config.defaults;
    let field_defaults = model::FieldDefaults {
        unit: defaults.unit,
        decimals: defaults.decimals,
        no_value: defaults.no_value,
        min: defaults.min,
        max: defaults.max,
        thresholds: defaults.thresholds.map(|thresholds| model::Thresholds {
            mode: thresholds.mode,
            steps: thresholds
                .steps
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
            .then(|| format!("{panel_path}.vizConfig.spec.fieldConfig.defaults.mappings")),
    };

    Ok(Some(model::Panel {
        kind: raw.spec.viz_config.group,
        title: raw.spec.title,
        source_path: path.to_string(),
        targets,
        grid: Some(grid),
        field_defaults: Some(field_defaults),
        reduce_options_path: viz_spec
            .options
            .reduce_options
            .is_some()
            .then(|| format!("{panel_path}.vizConfig.spec.options.reduceOptions")),
        transformations_path: (!raw.spec.data.spec.transformations.is_empty())
            .then(|| format!("{panel_path}.data.spec.transformations")),
    }))
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

#[derive(Deserialize)]
struct RawPanelElement {
    kind: String,
    spec: RawPanelSpec,
}

#[derive(Deserialize)]
struct RawPanelSpec {
    title: String,
    data: RawQueryGroup,
    #[serde(rename = "vizConfig")]
    viz_config: RawVizConfig,
}

#[derive(Deserialize)]
struct RawQueryGroup {
    #[allow(dead_code)]
    kind: String,
    spec: RawQueryGroupSpec,
}

#[derive(Deserialize)]
struct RawQueryGroupSpec {
    queries: Vec<RawPanelQuery>,
    transformations: Vec<Value>,
    #[serde(rename = "queryOptions")]
    _query_options: Value,
}

#[derive(Deserialize)]
struct RawPanelQuery {
    #[allow(dead_code)]
    kind: String,
    spec: RawPanelQuerySpec,
}

#[derive(Deserialize)]
struct RawPanelQuerySpec {
    hidden: bool,
    #[serde(rename = "refId")]
    _ref_id: String,
    query: RawDataQuery,
}

#[derive(Deserialize)]
struct RawDataQuery {
    #[allow(dead_code)]
    kind: String,
    group: String,
    spec: RawPrometheusQuery,
}

#[derive(Deserialize)]
struct RawPrometheusQuery {
    expr: Option<String>,
    #[serde(rename = "legendFormat")]
    legend_format: Option<String>,
    instant: Option<bool>,
}

#[derive(Deserialize)]
struct RawVizConfig {
    #[allow(dead_code)]
    kind: String,
    group: String,
    #[serde(rename = "version")]
    _version: String,
    spec: RawVizConfigSpec,
}

#[derive(Deserialize)]
struct RawVizConfigSpec {
    #[serde(rename = "fieldConfig")]
    field_config: RawFieldConfig,
    options: RawPanelOptions,
}

#[derive(Deserialize)]
struct RawFieldConfig {
    defaults: RawFieldDefaults,
}

#[derive(Deserialize)]
struct RawFieldDefaults {
    unit: Option<String>,
    decimals: Option<usize>,
    #[serde(rename = "noValue")]
    no_value: Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    thresholds: Option<RawThresholds>,
    custom: Option<RawGraphCustom>,
    mappings: Option<Value>,
}

#[derive(Deserialize)]
struct RawGraphCustom {
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

#[derive(Deserialize)]
struct RawStacking {
    mode: Option<String>,
}

#[derive(Deserialize)]
struct RawThresholdsStyle {
    mode: Option<String>,
}

#[derive(Deserialize)]
struct RawThresholds {
    mode: Option<String>,
    #[serde(default)]
    steps: Vec<RawThresholdStep>,
}

#[derive(Deserialize)]
struct RawThresholdStep {
    value: Option<f64>,
    color: Option<String>,
}

#[derive(Deserialize)]
struct RawPanelOptions {
    #[serde(rename = "reduceOptions")]
    reduce_options: Option<Value>,
}
