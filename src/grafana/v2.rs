use anyhow::{Context, Result, anyhow, ensure};
use serde::Deserialize;
use serde_json::Value;

use super::model;

mod autogrid;
mod conditions;
mod variables;

pub(super) const V2_API_VERSION: &str = "dashboard.grafana.app/v2";

/// Datasource plugin types whose queries are PromQL against a Prometheus-compatible API.
const PROMETHEUS_COMPATIBLE_GROUPS: &[&str] = &[
    "prometheus",
    "grafana-amazonprometheus-datasource",
    "grafana-azureprometheus-datasource",
];

fn is_prometheus_group(group: &str) -> bool {
    PROMETHEUS_COMPATIBLE_GROUPS.contains(&group)
}

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
    dashboard.variables = normalize_variables(spec, "spec", &mut dashboard.diagnostics)?;
    dashboard.layout = parse_layout(layout, elements, "spec.layout", &mut dashboard.diagnostics)?;
    dashboard.skipped_panels += dashboard
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "unsupported_element")
        .count();
    Ok(dashboard)
}

fn parse_layout(
    layout: &JsonObject,
    elements: &JsonObject,
    path: &str,
    diagnostics: &mut Vec<super::ImportDiagnostic>,
) -> Result<Vec<model::LayoutNode>> {
    match require_string_from(layout, "kind", &format!("{path}.kind"))? {
        "GridLayout" => parse_grid_layout(layout, elements, path, diagnostics),
        "RowsLayout" => parse_rows_layout(layout, elements, path, diagnostics),
        "TabsLayout" => parse_tabs_layout(layout, elements, path, diagnostics),
        "AutoGridLayout" => autogrid::parse_auto_grid_layout(layout, elements, path, diagnostics),
        kind => anyhow::bail!("unsupported Grafana V2 layout `{kind}` at {path}.kind"),
    }
}

fn parse_tabs_layout(
    layout: &JsonObject,
    elements: &JsonObject,
    path: &str,
    diagnostics: &mut Vec<super::ImportDiagnostic>,
) -> Result<Vec<model::LayoutNode>> {
    let spec_path = format!("{path}.spec");
    let spec = require_object_from(layout, "spec", &spec_path)?;
    let tabs_path = format!("{spec_path}.tabs");
    let tabs = require_array_from(spec, "tabs", &tabs_path)?;
    let mut normalized = Vec::new();
    for (index, tab) in tabs.iter().enumerate() {
        let tab_path = format!("{tabs_path}[{index}]");
        let tab = tab
            .as_object()
            .ok_or_else(|| anyhow!("invalid Grafana V2 tab at {tab_path}: expected an object"))?;
        require_expected_kind(tab, &tab_path, "TabsLayoutTab")?;
        let tab_spec_path = format!("{tab_path}.spec");
        let tab_spec = require_object_from(tab, "spec", &tab_spec_path)?;
        let title = optional_string_from(tab_spec, "title", &tab_spec_path)?.unwrap_or_default();
        for (field, description) in [
            ("repeat", "repeated tab"),
            ("conditionalRendering", "conditional tab rendering"),
        ] {
            ensure!(
                !tab_spec.contains_key(field),
                "unsupported Grafana V2 {description} at {tab_spec_path}.{field}"
            );
        }
        let variables = normalize_variables(tab_spec, &tab_spec_path, diagnostics)?;
        let child_path = format!("{tab_spec_path}.layout");
        let child_layout = require_object_from(tab_spec, "layout", &child_path)?;
        let children = parse_layout(child_layout, elements, &child_path, diagnostics)?;
        normalized.push(model::Tab {
            variables,
            title,
            source_path: tab_path,
            children,
        });
    }
    Ok(vec![model::LayoutNode::Tabs(model::Tabs {
        tabs: normalized,
    })])
}

fn parse_grid_layout(
    layout: &JsonObject,
    elements: &JsonObject,
    path: &str,
    diagnostics: &mut Vec<super::ImportDiagnostic>,
) -> Result<Vec<model::LayoutNode>> {
    let spec_path = format!("{path}.spec");
    let spec = require_object_from(layout, "spec", &spec_path)?;
    let items_path = format!("{spec_path}.items");
    let items = require_array_from(spec, "items", &items_path)?;
    let mut nodes = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("{items_path}[{index}]");
        let grid = parse_grid_item(item, &item_path)?;
        let element_path = format!("spec.elements[{:?}]", grid.element_name);
        let element = elements.get(&grid.element_name).ok_or_else(|| {
            anyhow!(
                "unresolved Grafana V2 element reference `{}` at {item_path}.spec.element.name",
                grid.element_name
            )
        })?;
        if let Some(panel) = parse_panel(element, &element_path, Some(grid.position), diagnostics)?
        {
            nodes.push(model::LayoutNode::Panel(panel));
        }
    }
    Ok(nodes)
}

fn parse_rows_layout(
    layout: &JsonObject,
    elements: &JsonObject,
    path: &str,
    diagnostics: &mut Vec<super::ImportDiagnostic>,
) -> Result<Vec<model::LayoutNode>> {
    let spec_path = format!("{path}.spec");
    let spec = require_object_from(layout, "spec", &spec_path)?;
    let rows_path = format!("{spec_path}.rows");
    let rows = require_array_from(spec, "rows", &rows_path)?;
    let mut nodes = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let row_path = format!("{rows_path}[{index}]");
        let row = row
            .as_object()
            .ok_or_else(|| anyhow!("invalid Grafana V2 row at {row_path}: expected an object"))?;
        require_expected_kind(row, &row_path, "RowsLayoutRow")?;
        let row_spec_path = format!("{row_path}.spec");
        let row_spec = require_object_from(row, "spec", &row_spec_path)?;
        let title = optional_string_from(row_spec, "title", &row_spec_path)?.unwrap_or_default();
        let collapsed = optional_bool_from(row_spec, "collapse", &row_spec_path)?;
        let hidden_header = optional_bool_from(row_spec, "hideHeader", &row_spec_path)?;

        for (field, description) in [
            ("repeat", "repeated row"),
            ("conditionalRendering", "conditional row rendering"),
        ] {
            ensure!(
                !row_spec.contains_key(field),
                "unsupported Grafana V2 {description} at {row_spec_path}.{field}"
            );
        }

        let variables = normalize_variables(row_spec, &row_spec_path, diagnostics)?;

        // `fillScreen` stretches a row to the browser viewport; terminal rows already
        // size to their content, so the flag is validated and otherwise ignored.
        optional_bool_from(row_spec, "fillScreen", &row_spec_path)?;

        let child_path = format!("{row_spec_path}.layout");
        let child_layout = require_object_from(row_spec, "layout", &child_path)?;
        let children = parse_layout(child_layout, elements, &child_path, diagnostics)?;
        nodes.push(model::LayoutNode::Row(model::Row {
            variables,
            title,
            collapsed,
            hidden_header,
            source_path: row_path,
            children,
        }));
    }
    Ok(nodes)
}

fn normalize_variables(
    dashboard_spec: &JsonObject,
    parent_path: &str,
    diagnostics: &mut Vec<super::ImportDiagnostic>,
) -> Result<Vec<model::Variable>> {
    let variables_path = format!("{parent_path}.variables");
    let variables = optional_array_from(dashboard_spec, "variables", &variables_path)?;

    let mut normalized = Vec::new();
    for (index, variable) in variables.iter().enumerate() {
        let path = format!("{parent_path}.variables[{index}]");
        let variable = variable
            .as_object()
            .ok_or_else(|| anyhow!("invalid Grafana V2 variable at {path}: expected an object"))?;
        let kind = require_string_from(variable, "kind", &format!("{path}.kind"))?;
        let spec = require_object_from(variable, "spec", &format!("{path}.spec"))?;
        let variable = match kind {
            "QueryVariable" => normalize_query_variable(spec, &path, diagnostics)?,
            "TextVariable" | "ConstantVariable" | "DatasourceVariable" | "IntervalVariable"
            | "CustomVariable" | "GroupByVariable" => Some(normalize_option_variable(spec, &path)?),
            "SwitchVariable" => Some(normalize_switch_variable(spec, &path)?),
            "AdhocVariable" => {
                diagnostics.push(super::ImportDiagnostic::new(
                    "unsupported_variable",
                    &path,
                    "unsupported Grafana V2 variable kind `AdhocVariable` skipped",
                ));
                None
            }
            other => {
                diagnostics.push(super::ImportDiagnostic::new(
                    "unsupported_variable",
                    &path,
                    format!("unsupported Grafana V2 variable kind `{other}` skipped"),
                ));
                None
            }
        };
        if let Some(mut variable) = variable {
            let query = variable
                .query
                .as_ref()
                .zip(variable.query_path.as_ref())
                .filter(|_| variable.kind.as_deref() == Some("query"))
                .map(|(query, path)| super::TemplateQueryVar {
                    name: variable.name.clone(),
                    query: query.clone(),
                    regex: variable.regex.clone(),
                    query_path: path.clone(),
                });
            variable.retained = Some(variables::retain(
                spec,
                kind,
                &format!("{path}.spec"),
                query,
            )?);
            normalized.push(variable);
        }
    }
    Ok(normalized)
}

fn normalize_query_variable(
    spec: &JsonObject,
    source_path: &str,
    diagnostics: &mut Vec<super::ImportDiagnostic>,
) -> Result<Option<model::Variable>> {
    let source_path = source_path.to_string();
    let name = require_string_from(spec, "name", &format!("{source_path}.spec.name"))?.to_string();
    let query_path = format!("{source_path}.spec.query");
    let query = require_object_from(spec, "query", &query_path)?;
    require_expected_kind(query, &query_path, "DataQuery")?;
    let datasource = require_string_from(query, "group", &format!("{query_path}.group"))?;
    let is_prometheus = is_prometheus_group(datasource);
    if !is_prometheus {
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
        retained: None,
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

fn normalize_option_variable(spec: &JsonObject, source_path: &str) -> Result<model::Variable> {
    let source_path = source_path.to_string();
    Ok(model::Variable {
        retained: None,
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

fn normalize_switch_variable(spec: &JsonObject, source_path: &str) -> Result<model::Variable> {
    let source_path = source_path.to_string();
    Ok(model::Variable {
        retained: None,
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

/// Returns the array at `key`, treating an absent or `null` value as empty.
///
/// Grafana's resource API serializes unset slices as `null`, and its frontend
/// exporter drops empty fields, so neither can be distinguished from `[]`.
fn optional_array_from<'a>(object: &'a JsonObject, key: &str, path: &str) -> Result<&'a [Value]> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(values)) => Ok(values),
        Some(_) => anyhow::bail!("invalid Grafana V2 resource at {path}: expected an array"),
    }
}

/// Returns the object at `key`, treating an absent or `null` value as `None`.
fn optional_object_from<'a>(
    object: &'a JsonObject,
    key: &str,
    path: &str,
) -> Result<Option<&'a JsonObject>> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(value)) => Ok(Some(value)),
        Some(_) => anyhow::bail!("invalid Grafana V2 resource at {path}: expected an object"),
    }
}

fn optional_string_from(object: &JsonObject, key: &str, path: &str) -> Result<Option<String>> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => anyhow::bail!("invalid Grafana V2 resource at {path}.{key}: expected a string"),
    }
}

fn require_string_from<'a>(object: &'a JsonObject, key: &str, path: &str) -> Result<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("invalid Grafana V2 resource at {path}: expected a string"))
}

fn optional_bool_from(object: &JsonObject, key: &str, path: &str) -> Result<bool> {
    match object.get(key) {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => anyhow::bail!("invalid Grafana V2 resource at {path}.{key}: expected a boolean"),
    }
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
    grid: Option<model::GridPos>,
    diagnostics: &mut Vec<super::ImportDiagnostic>,
) -> Result<Option<model::Panel>> {
    let element = value
        .as_object()
        .ok_or_else(|| anyhow!("invalid Grafana V2 element at {path}: expected an object"))?;
    let kind_path = format!("{path}.kind");
    let kind = require_string_from(element, "kind", &kind_path)?;
    if kind != "Panel" {
        let message = if kind == "LibraryPanel" {
            "Grafana V2 library panel skipped: exports reference library panels by uid only; \
             re-export with \"Share dashboard with another instance\" enabled to inline them"
                .to_string()
        } else {
            format!("unsupported Grafana V2 element kind `{kind}` skipped")
        };
        diagnostics.push(super::ImportDiagnostic::new(
            "unsupported_element",
            path,
            message,
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
        if !is_prometheus_group(&query.spec.query.group) {
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
        grid,
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

/// Validates the shape of an inline V2 panel before typed deserialization.
///
/// Only `vizConfig.group` and each query's `group` are required: without them the
/// renderer and datasource are unknown. Every other container may be absent or
/// `null` in real exports and is defaulted, but a present value of the wrong type
/// is still rejected at its native path.
fn validate_panel_structure(element: &JsonObject, path: &str) -> Result<()> {
    let panel_path = format!("{path}.spec");
    let panel = require_object_from(element, "spec", &panel_path)?;
    let id_path = format!("{panel_path}.id");
    ensure!(
        panel
            .get("id")
            .is_none_or(|id| id.is_null() || id.is_number()),
        "invalid Grafana V2 resource at {id_path}: expected a number"
    );
    optional_string_from(panel, "title", &panel_path)?;
    optional_array_from(panel, "links", &format!("{panel_path}.links"))?;

    let data_path = format!("{panel_path}.data");
    if let Some(data) = optional_object_from(panel, "data", &data_path)? {
        // Grafana serializes the query group of a query-less panel (e.g. `text`)
        // as its Go zero value, with an empty `kind`.
        if data.get("kind").and_then(Value::as_str) != Some("") {
            require_expected_kind(data, &data_path, "QueryGroup")?;
        }
        let data_spec_path = format!("{data_path}.spec");
        if let Some(data_spec) = optional_object_from(data, "spec", &data_spec_path)? {
            validate_query_group_spec(data_spec, &data_spec_path)?;
        }
    }

    let viz_path = format!("{panel_path}.vizConfig");
    let viz = require_object_from(panel, "vizConfig", &viz_path)?;
    require_expected_kind(viz, &viz_path, "VizConfig")?;
    require_string_from(viz, "group", &format!("{viz_path}.group"))?;
    optional_string_from(viz, "version", &viz_path)?;
    let viz_spec_path = format!("{viz_path}.spec");
    if let Some(viz_spec) = optional_object_from(viz, "spec", &viz_spec_path)? {
        let field_config_path = format!("{viz_spec_path}.fieldConfig");
        if let Some(field_config) =
            optional_object_from(viz_spec, "fieldConfig", &field_config_path)?
        {
            optional_object_from(
                field_config,
                "defaults",
                &format!("{field_config_path}.defaults"),
            )?;
            optional_array_from(
                field_config,
                "overrides",
                &format!("{field_config_path}.overrides"),
            )?;
        }
        optional_object_from(viz_spec, "options", &format!("{viz_spec_path}.options"))?;
    }
    Ok(())
}

fn validate_query_group_spec(data_spec: &JsonObject, data_spec_path: &str) -> Result<()> {
    let queries_path = format!("{data_spec_path}.queries");
    let queries = optional_array_from(data_spec, "queries", &queries_path)?;
    optional_array_from(
        data_spec,
        "transformations",
        &format!("{data_spec_path}.transformations"),
    )?;
    optional_object_from(
        data_spec,
        "queryOptions",
        &format!("{data_spec_path}.queryOptions"),
    )?;
    for (index, query) in queries.iter().enumerate() {
        let query_path = format!("{queries_path}[{index}]");
        let query = query.as_object().ok_or_else(|| {
            anyhow!("invalid Grafana V2 panel query at {query_path}: expected an object")
        })?;
        require_expected_kind(query, &query_path, "PanelQuery")?;
        let query_spec_path = format!("{query_path}.spec");
        let query_spec = require_object_from(query, "spec", &query_spec_path)?;
        optional_bool_from(query_spec, "hidden", &query_spec_path)?;
        optional_string_from(query_spec, "refId", &query_spec_path)?;
        let data_query_path = format!("{query_spec_path}.query");
        let data_query = require_object_from(query_spec, "query", &data_query_path)?;
        require_expected_kind(data_query, &data_query_path, "DataQuery")?;
        require_string_from(data_query, "group", &format!("{data_query_path}.group"))?;
        optional_object_from(data_query, "spec", &format!("{data_query_path}.spec"))?;
    }
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

/// Deserializes an absent or `null` field as its type's default.
///
/// Pair with `#[serde(default)]` so missing fields also take the default.
fn null_as_default<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Deserialize)]
struct RawPanelElement {
    spec: RawPanelSpec,
}

#[derive(Deserialize)]
struct RawPanelSpec {
    #[serde(default, deserialize_with = "null_as_default")]
    title: String,
    #[serde(default, deserialize_with = "null_as_default")]
    data: RawQueryGroup,
    #[serde(rename = "vizConfig")]
    viz_config: RawVizConfig,
}

#[derive(Default, Deserialize)]
struct RawQueryGroup {
    #[serde(default, deserialize_with = "null_as_default")]
    spec: RawQueryGroupSpec,
}

#[derive(Default, Deserialize)]
struct RawQueryGroupSpec {
    #[serde(default, deserialize_with = "null_as_default")]
    queries: Vec<RawPanelQuery>,
    #[serde(default, deserialize_with = "null_as_default")]
    transformations: Vec<Value>,
}

#[derive(Deserialize)]
struct RawPanelQuery {
    spec: RawPanelQuerySpec,
}

#[derive(Deserialize)]
struct RawPanelQuerySpec {
    #[serde(default, deserialize_with = "null_as_default")]
    hidden: bool,
    query: RawDataQuery,
}

#[derive(Deserialize)]
struct RawDataQuery {
    group: String,
    #[serde(default, deserialize_with = "null_as_default")]
    spec: RawPrometheusQuery,
}

#[derive(Default, Deserialize)]
struct RawPrometheusQuery {
    expr: Option<String>,
    #[serde(rename = "legendFormat")]
    legend_format: Option<String>,
    instant: Option<bool>,
}

#[derive(Deserialize)]
struct RawVizConfig {
    group: String,
    #[serde(default, deserialize_with = "null_as_default")]
    spec: RawVizConfigSpec,
}

#[derive(Default, Deserialize)]
struct RawVizConfigSpec {
    #[serde(rename = "fieldConfig", default, deserialize_with = "null_as_default")]
    field_config: RawFieldConfig,
    #[serde(default, deserialize_with = "null_as_default")]
    options: RawPanelOptions,
}

#[derive(Default, Deserialize)]
struct RawFieldConfig {
    #[serde(default, deserialize_with = "null_as_default")]
    defaults: RawFieldDefaults,
}

#[derive(Default, Deserialize)]
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
    #[serde(default, deserialize_with = "null_as_default")]
    steps: Vec<RawThresholdStep>,
}

#[derive(Deserialize)]
struct RawThresholdStep {
    value: Option<f64>,
    color: Option<String>,
}

#[derive(Default, Deserialize)]
struct RawPanelOptions {
    #[serde(rename = "reduceOptions")]
    reduce_options: Option<Value>,
}
