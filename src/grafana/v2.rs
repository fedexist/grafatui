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

#[derive(Deserialize)]
struct RawVariableOption {
    text: Option<Value>,
    value: Option<Value>,
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
    dashboard.refresh = match spec.get("timeSettings") {
        None => None,
        Some(Value::Object(settings)) => match settings.get("autoRefresh") {
            None => None,
            Some(Value::String(refresh)) => Some(refresh.clone()),
            Some(_) => anyhow::bail!(
                "invalid Grafana V2 auto refresh at spec.timeSettings.autoRefresh: expected a string"
            ),
        },
        Some(_) => anyhow::bail!(
            "invalid Grafana V2 time settings at spec.timeSettings: expected an object"
        ),
    };
    dashboard.variables = normalize_variables(spec, &mut dashboard.diagnostics)?;
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
            dashboard.layout.push(model::LayoutNode::Panel(panel));
        } else {
            dashboard.skipped_panels += 1;
        }
    }
    Ok(dashboard)
}

fn normalize_variables(
    dashboard_spec: &JsonObject,
    diagnostics: &mut Vec<super::ImportDiagnostic>,
) -> Result<Vec<model::Variable>> {
    let variables = match dashboard_spec.get("variables") {
        None => return Ok(Vec::new()),
        Some(Value::Array(variables)) => variables,
        Some(_) => {
            anyhow::bail!("invalid Grafana V2 variables at spec.variables: expected an array")
        }
    };

    let mut normalized = Vec::new();
    for (index, variable) in variables.iter().enumerate() {
        let path = format!("spec.variables[{index}]");
        let variable = variable
            .as_object()
            .ok_or_else(|| anyhow!("invalid Grafana V2 variable at {path}: expected an object"))?;
        let kind = require_string_from(variable, "kind", &format!("{path}.kind"))?;
        let spec = require_object_from(variable, "spec", &format!("{path}.spec"))?;
        let variable = match kind {
            "QueryVariable" => normalize_query_variable(spec, index, diagnostics)?,
            "TextVariable" | "ConstantVariable" | "DatasourceVariable" | "IntervalVariable"
            | "CustomVariable" | "GroupByVariable" => Some(normalize_option_variable(spec, index)?),
            "SwitchVariable" => Some(normalize_switch_variable(spec, index)?),
            "AdhocVariable" => {
                diagnostics.push(super::ImportDiagnostic::new(
                    "unsupported_variable",
                    path,
                    "unsupported Grafana V2 variable kind `AdhocVariable` skipped",
                ));
                None
            }
            other => {
                diagnostics.push(super::ImportDiagnostic::new(
                    "unsupported_variable",
                    path,
                    format!("unsupported Grafana V2 variable kind `{other}` skipped"),
                ));
                None
            }
        };
        if let Some(variable) = variable {
            normalized.push(variable);
        }
    }
    Ok(normalized)
}

fn normalize_query_variable(
    spec: &JsonObject,
    index: usize,
    diagnostics: &mut Vec<super::ImportDiagnostic>,
) -> Result<Option<model::Variable>> {
    let source_path = format!("spec.variables[{index}]");
    let name = require_string_from(spec, "name", &format!("{source_path}.spec.name"))?.to_string();
    let query_path = format!("{source_path}.spec.query");
    let query = require_object_from(spec, "query", &query_path)?;
    require_expected_kind(query, &query_path, "DataQuery")?;
    let datasource = require_string_from(query, "group", &format!("{query_path}.group"))?;
    let is_prometheus = datasource == "prometheus";
    if datasource != "prometheus" {
        diagnostics.push(super::ImportDiagnostic::new(
            "unsupported_datasource",
            &query_path,
            format!("unsupported Grafana V2 datasource `{datasource}` skipped"),
        ));
    }
    let query_spec_path = format!("{query_path}.spec");
    let query_spec = require_object_from(query, "spec", &query_spec_path)?;
    let query = query_spec
        .get("query")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .map(|query| {
            (
                query.to_string(),
                format!("{source_path}.spec.query.spec.query"),
            )
        })
        .or_else(|| {
            spec.get("definition")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|definition| !definition.is_empty())
                .map(|definition| {
                    (
                        definition.to_string(),
                        format!("{source_path}.spec.definition"),
                    )
                })
        });

    Ok(Some(model::Variable {
        name,
        kind: is_prometheus.then_some("query".to_string()),
        current: current_from_option(spec),
        query: query.as_ref().map(|(query, _)| query.clone()),
        regex: optional_string(spec, "regex"),
        all_value: optional_string(spec, "allValue"),
        source_path,
        query_path: query.map(|(_, path)| path),
    }))
}

fn normalize_option_variable(spec: &JsonObject, index: usize) -> Result<model::Variable> {
    let source_path = format!("spec.variables[{index}]");
    Ok(model::Variable {
        name: require_string_from(spec, "name", &format!("{source_path}.spec.name"))?.to_string(),
        kind: None,
        current: current_from_option(spec),
        query: None,
        regex: None,
        all_value: optional_string(spec, "allValue"),
        source_path,
        query_path: None,
    })
}

fn normalize_switch_variable(spec: &JsonObject, index: usize) -> Result<model::Variable> {
    let source_path = format!("spec.variables[{index}]");
    Ok(model::Variable {
        name: require_string_from(spec, "name", &format!("{source_path}.spec.name"))?.to_string(),
        kind: None,
        current: spec.get("current").and_then(Value::as_str).map(|current| {
            model::VariableCurrent {
                text: None,
                value: Some(Value::String(current.to_string())),
            }
        }),
        query: None,
        regex: None,
        all_value: None,
        source_path,
        query_path: None,
    })
}

fn current_from_option(spec: &JsonObject) -> Option<model::VariableCurrent> {
    let option = spec
        .get("current")
        .filter(|current| current.is_object())
        .and_then(|current| serde_json::from_value::<RawVariableOption>(current.clone()).ok())?;
    Some(model::VariableCurrent {
        text: option.text,
        value: option.value,
    })
}

fn optional_string(spec: &JsonObject, key: &str) -> Option<String> {
    spec.get(key).and_then(Value::as_str).map(str::to_string)
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

fn require_bool_from(object: &JsonObject, key: &str, path: &str) -> Result<bool> {
    object
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| anyhow!("invalid Grafana V2 resource at {path}: expected a boolean"))
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
    diagnostics: &mut Vec<super::ImportDiagnostic>,
) -> Result<Option<model::Panel>> {
    let element = value
        .as_object()
        .ok_or_else(|| anyhow!("invalid Grafana V2 element at {path}: expected an object"))?;
    let kind_path = format!("{path}.kind");
    let kind = require_string_from(element, "kind", &kind_path)?;
    if kind != "Panel" {
        diagnostics.push(super::ImportDiagnostic::new(
            "unsupported_element",
            path,
            format!("unsupported Grafana V2 element kind `{kind}` skipped"),
        ));
        return Ok(None);
    }
    validate_panel_structure(element, path)?;

    let raw: RawPanelElement = serde_json::from_value(value.clone())
        .with_context(|| format!("parsing Grafana V2 panel at {path}"))?;

    let panel_path = format!("{path}.spec");
    let data_path = format!("{panel_path}.data.spec.queries");
    let mut targets = Vec::new();
    let mut has_visible_target = false;
    let mut has_supported_visible_target = false;
    for (index, query) in raw.spec.data.spec.queries.into_iter().enumerate() {
        let query_path = format!("{data_path}[{index}].spec.query");
        if !query.spec.hidden {
            has_visible_target = true;
        }
        if query.spec.query.group != "prometheus" {
            diagnostics.push(super::ImportDiagnostic::new(
                "unsupported_datasource",
                &query_path,
                format!(
                    "unsupported Grafana V2 datasource `{}` skipped",
                    query.spec.query.group
                ),
            ));
            continue;
        }
        if !query.spec.hidden {
            has_supported_visible_target = true;
        }

        let expr_path = format!("{query_path}.spec.expr");
        let expr = query
            .spec
            .query
            .spec
            .expr
            .filter(|expr| !expr.trim().is_empty());
        if !query.spec.hidden && expr.is_none() {
            diagnostics.push(super::ImportDiagnostic::new(
                "missing_query_expression",
                &expr_path,
                "visible Prometheus query has no expression and was skipped",
            ));
        }
        targets.push(model::Target {
            expr,
            expr_path,
            legend_format: query.spec.query.spec.legend_format,
            instant: query.spec.query.spec.instant,
            hidden: query.spec.hidden,
        });
    }
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
        count_as_skipped_if_empty: has_visible_target && !has_supported_visible_target,
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

fn validate_panel_structure(element: &JsonObject, path: &str) -> Result<()> {
    let panel_path = format!("{path}.spec");
    let panel = require_object_from(element, "spec", &panel_path)?;
    let id_path = format!("{panel_path}.id");
    ensure!(
        panel.get("id").is_some_and(Value::is_number),
        "invalid Grafana V2 resource at {id_path}: expected a number"
    );
    require_string_from(panel, "title", &format!("{panel_path}.title"))?;
    require_array_from(panel, "links", &format!("{panel_path}.links"))?;

    let data_path = format!("{panel_path}.data");
    let data = require_object_from(panel, "data", &data_path)?;
    require_expected_kind(data, &data_path, "QueryGroup")?;
    let data_spec_path = format!("{data_path}.spec");
    let data_spec = require_object_from(data, "spec", &data_spec_path)?;
    let queries_path = format!("{data_spec_path}.queries");
    let queries = require_array_from(data_spec, "queries", &queries_path)?;
    require_array_from(
        data_spec,
        "transformations",
        &format!("{data_spec_path}.transformations"),
    )?;
    for (index, query) in queries.iter().enumerate() {
        let query_path = format!("{queries_path}[{index}]");
        let query = query.as_object().ok_or_else(|| {
            anyhow!("invalid Grafana V2 panel query at {query_path}: expected an object")
        })?;
        require_expected_kind(query, &query_path, "PanelQuery")?;
        let query_spec_path = format!("{query_path}.spec");
        let query_spec = require_object_from(query, "spec", &query_spec_path)?;
        require_bool_from(query_spec, "hidden", &format!("{query_spec_path}.hidden"))?;
        let data_query_path = format!("{query_spec_path}.query");
        let data_query = require_object_from(query_spec, "query", &data_query_path)?;
        require_expected_kind(data_query, &data_query_path, "DataQuery")?;
        require_string_from(data_query, "group", &format!("{data_query_path}.group"))?;
        require_object_from(data_query, "spec", &format!("{data_query_path}.spec"))?;
    }

    let viz_path = format!("{panel_path}.vizConfig");
    let viz = require_object_from(panel, "vizConfig", &viz_path)?;
    require_expected_kind(viz, &viz_path, "VizConfig")?;
    require_string_from(viz, "group", &format!("{viz_path}.group"))?;
    let viz_spec_path = format!("{viz_path}.spec");
    let viz_spec = require_object_from(viz, "spec", &viz_spec_path)?;
    let field_config_path = format!("{viz_spec_path}.fieldConfig");
    let field_config = require_object_from(viz_spec, "fieldConfig", &field_config_path)?;
    require_object_from(
        field_config,
        "defaults",
        &format!("{field_config_path}.defaults"),
    )?;
    require_object_from(viz_spec, "options", &format!("{viz_spec_path}.options"))?;
    Ok(())
}

fn require_expected_kind(object: &JsonObject, path: &str, expected: &str) -> Result<()> {
    let kind_path = format!("{path}.kind");
    let kind = require_string_from(object, "kind", &kind_path)?;
    ensure!(
        kind == expected,
        "invalid Grafana V2 kind `{kind}` at {kind_path}: expected `{expected}`"
    );
    Ok(())
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
