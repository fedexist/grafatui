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
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

mod classic;
mod import;
mod model;
mod v2;

/// Result of importing a Grafana dashboard.
#[derive(Debug, Clone, Default)]
pub(crate) struct DashboardImport {
    /// Dashboard title.
    pub(crate) title: String,
    /// List of panels extracted.
    pub(crate) queries: Vec<QueryPanel>,
    /// Recursive layout of panels and rows.
    pub(crate) layout: crate::dashboard::DashboardLayout,
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

pub(crate) fn load_grafana_dashboard(path: &std::path::Path) -> Result<DashboardImport> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("reading grafana dashboard: {}", path.display()))?;
    parse_grafana_dashboard(&data)
}

fn parse_grafana_dashboard(data: &str) -> Result<DashboardImport> {
    let value = serde_json::from_str(data).context("parsing Grafana dashboard JSON")?;
    import::finish(detect_and_adapt(value)?)
}

fn detect_and_adapt(value: Value) -> Result<model::Dashboard> {
    match value.get("apiVersion") {
        None => {
            if value.as_object().is_some_and(|object| {
                ["kind", "spec", "metadata", "status"]
                    .iter()
                    .any(|marker| object.contains_key(*marker))
            }) {
                anyhow::bail!(
                    "invalid Grafana dashboard resource at apiVersion: missing required field `apiVersion`"
                );
            }
            classic::adapt(value)
        }
        Some(Value::String(version)) if version == v2::V2_API_VERSION => v2::adapt(value),
        Some(Value::String(version)) => anyhow::bail!(
            "unsupported Grafana dashboard resource apiVersion `{version}` at apiVersion; supported resource version is `{}`",
            v2::V2_API_VERSION
        ),
        Some(_) => anyhow::bail!(
            "invalid Grafana dashboard resource apiVersion at apiVersion: expected a string"
        ),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_model_panel(kind: &str, expr: Option<&str>) -> model::LayoutNode {
        model::LayoutNode::Panel(model::Panel {
            kind: kind.into(),
            title: kind.into(),
            source_path: format!("layout.{kind}"),
            targets: expr
                .into_iter()
                .map(|expr| model::Target {
                    expr: Some(expr.into()),
                    expr_path: format!("layout.{kind}.expr"),
                    legend_format: None,
                    instant: None,
                    hidden: false,
                })
                .collect(),
            count_as_skipped_if_empty: false,
            grid: None,
            field_defaults: None,
            reduce_options_path: None,
            transformations_path: None,
        })
    }

    #[test]
    fn normalized_layout_references_only_imported_panel_indices() {
        let dashboard = model::Dashboard {
            title: "Rows".into(),
            layout: vec![model::LayoutNode::Row(model::Row {
                title: "Group".into(),
                collapsed: false,
                hidden_header: false,
                source_path: "layout.row".into(),
                children: vec![
                    test_model_panel("piechart", None),
                    test_model_panel("timeseries", Some("up")),
                ],
            })],
            ..model::Dashboard::default()
        };
        let imported = import::finish(dashboard).unwrap();
        assert_eq!(imported.queries.len(), 1);
        assert_eq!(imported.layout.visible_panel_indices(), vec![0]);
        assert_eq!(imported.skipped_panels, 1);
    }

    fn minimal_v2_with_layout(layout: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "apiVersion": "dashboard.grafana.app/v2",
            "kind": "Dashboard",
            "metadata": {"name": "test"},
            "spec": {
                "title": "Test",
                "elements": {},
                "layout": layout,
                "variables": [],
                "timeSettings": {"from": "now-6h", "to": "now", "autoRefresh": ""}
            },
            "status": {}
        })
    }

    fn valid_v2_resource() -> serde_json::Value {
        serde_json::json!({
            "apiVersion": "dashboard.grafana.app/v2",
            "kind": "Dashboard",
            "spec": {
                "title": "Test",
                "elements": {
                    "panel-1": {
                        "kind": "Panel",
                        "spec": {
                            "id": 1,
                            "title": "Panel",
                            "links": [],
                            "data": {"kind": "QueryGroup", "spec": {"queries": [], "transformations": [], "queryOptions": {}}},
                            "vizConfig": {"kind": "VizConfig", "group": "timeseries", "version": "v0", "spec": {"fieldConfig": {"defaults": {}}, "options": {}}}
                        }
                    }
                },
                "layout": {"kind": "GridLayout", "spec": {"items": [{
                    "kind": "GridLayoutItem",
                    "spec": {"x": 0, "y": 0, "width": 1, "height": 1, "element": {"kind": "ElementReference", "name": "panel-1"}}
                }]}},
                "variables": [],
                "timeSettings": {"from": "now-6h", "to": "now", "autoRefresh": ""}
            }
        })
    }

    fn v2_row_resource_with_field(field: &str, value: serde_json::Value) -> serde_json::Value {
        let mut json = valid_v2_resource();
        let mut row_spec = serde_json::json!({
            "title": "Row",
            "layout": {"kind": "GridLayout", "spec": {"items": []}}
        });
        row_spec
            .as_object_mut()
            .unwrap()
            .insert(field.into(), value);
        json["spec"]["layout"] = serde_json::json!({
            "kind": "RowsLayout",
            "spec": {
                "rows": [{"kind": "RowsLayoutRow", "spec": row_spec}]
            }
        });
        json
    }

    fn make_v2_panel_importable(json: &mut serde_json::Value) {
        json["spec"]["elements"]["panel-1"]["spec"]["data"]["spec"]["queries"] = serde_json::json!([{
            "kind": "PanelQuery",
            "spec": {
                "hidden": false,
                "refId": "A",
                "query": {
                    "kind": "DataQuery",
                    "group": "prometheus",
                    "spec": {"expr": "up"}
                }
            }
        }]);
    }

    #[test]
    fn v2_fixed_grid_panel_matches_classic_semantics() {
        let classic = parse_grafana_dashboard(include_str!(
            "../tests/fixtures/grafana/classic_compatibility.json"
        ))
        .unwrap();
        let v2 = parse_grafana_dashboard(include_str!(
            "../tests/fixtures/grafana/v2_compatibility.json"
        ))
        .unwrap();

        assert_eq!(v2.title, classic.title);
        assert_eq!(v2.refresh_rate_ms, classic.refresh_rate_ms);
        assert_eq!(v2.vars, classic.vars);
        assert_eq!(v2.query_vars.len(), 1);
        assert_eq!(v2.query_vars[0].name, classic.query_vars[0].name);
        assert_eq!(v2.query_vars[0].query, classic.query_vars[0].query);
        assert_eq!(v2.query_vars[0].regex, classic.query_vars[0].regex);
        assert_eq!(
            v2.query_vars[0].query_path,
            "spec.variables[0].spec.query.spec.query"
        );
        assert_eq!(v2.queries.len(), 1);
        let (actual, expected) = (&v2.queries[0], &classic.queries[0]);
        assert_eq!(actual.title, expected.title);
        assert_eq!(actual.exprs, expected.exprs);
        assert_eq!(actual.legends, expected.legends);
        assert_eq!(actual.query_modes, expected.query_modes);
        assert_eq!(actual.panel_type, expected.panel_type);
        assert_eq!(actual.display, expected.display);
        assert_eq!(actual.options, expected.options);
        let (actual_thresholds, expected_thresholds) = (
            actual.thresholds.as_ref().unwrap(),
            expected.thresholds.as_ref().unwrap(),
        );
        assert_eq!(actual_thresholds.mode, expected_thresholds.mode);
        assert_eq!(actual_thresholds.style, expected_thresholds.style);
        assert_eq!(
            actual_thresholds.steps.len(),
            expected_thresholds.steps.len()
        );
        for (actual_step, expected_step) in actual_thresholds
            .steps
            .iter()
            .zip(&expected_thresholds.steps)
        {
            assert_eq!(actual_step.value, expected_step.value);
            assert_eq!(actual_step.color, expected_step.color);
        }
        assert_eq!((actual.min, actual.max), (expected.min, expected.max));
        assert_eq!(actual.autogrid, expected.autogrid);
        assert_eq!(
            actual.grid.map(|grid| (grid.x, grid.y, grid.w, grid.h)),
            expected.grid.map(|grid| (grid.x, grid.y, grid.w, grid.h))
        );
        assert_eq!(
            actual.expr_paths,
            ["spec.elements[\"panel-1\"].spec.data.spec.queries[1].spec.query.spec.expr"]
        );
    }

    #[test]
    fn rejects_unsupported_resource_versions() {
        for version in [
            "dashboard.grafana.app/v1",
            "dashboard.grafana.app/v2alpha1",
            "dashboard.grafana.app/v2beta1",
        ] {
            let json = format!(r#"{{"apiVersion":"{version}","kind":"Dashboard","spec":{{}}}}"#);
            let error = parse_grafana_dashboard(&json).unwrap_err().to_string();
            assert!(error.contains(version));
            assert!(error.contains("apiVersion"));
        }
    }

    #[test]
    fn rejects_resource_envelope_without_api_version() {
        let error = parse_grafana_dashboard(
            r#"{"kind":"Dashboard","spec":{"title":"Incomplete resource"}}"#,
        )
        .expect_err("resource-shaped JSON without apiVersion must be rejected")
        .to_string();

        assert!(
            error.contains("apiVersion"),
            "expected missing apiVersion error, got `{error}`"
        );
    }

    #[test]
    fn rejects_unsupported_v2_layouts() {
        for kind in ["TabsLayout", "AutoGridLayout"] {
            let json = minimal_v2_with_layout(serde_json::json!({"kind": kind, "spec": {}}));
            let error = parse_grafana_dashboard(&json.to_string())
                .unwrap_err()
                .to_string();
            assert!(error.contains(kind));
            assert!(error.contains("spec.layout.kind"));
        }
    }

    #[test]
    fn v2_rows_import_expanded_collapsed_and_nested_content() {
        let dashboard = parse_grafana_dashboard(include_str!(
            "../tests/fixtures/grafana/v2_rows_layout.json"
        ))
        .unwrap();

        assert_eq!(dashboard.queries.len(), 3);
        assert_eq!(dashboard.layout.visible_panel_indices(), vec![0]);
    }

    #[test]
    fn v2_rows_hidden_header_is_transparent_even_when_collapsed() {
        let mut json = valid_v2_resource();
        make_v2_panel_importable(&mut json);
        let grid = json["spec"]["layout"].clone();
        json["spec"]["layout"] = serde_json::json!({
            "kind": "RowsLayout",
            "spec": {"rows": [{
                "kind": "RowsLayoutRow",
                "spec": {
                    "title": "Hidden",
                    "collapse": true,
                    "hideHeader": true,
                    "layout": grid
                }
            }]}
        });

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();

        assert_eq!(dashboard.queries.len(), 1);
        assert_eq!(dashboard.layout.visible_panel_indices(), vec![0]);
        assert_eq!(dashboard.layout.visible_items().len(), 1);
    }

    #[test]
    fn v2_rows_reject_deferred_semantics_at_native_paths() {
        for (field, value) in [
            (
                "repeat",
                serde_json::json!({"mode": "variable", "value": "job"}),
            ),
            (
                "conditionalRendering",
                serde_json::json!({"kind": "ConditionalRenderingGroup", "spec": {}}),
            ),
            (
                "variables",
                serde_json::json!([{"kind": "TextVariable", "spec": {"name": "x"}}]),
            ),
            ("fillScreen", serde_json::json!(true)),
        ] {
            let error =
                parse_grafana_dashboard(&v2_row_resource_with_field(field, value).to_string())
                    .unwrap_err()
                    .to_string();
            assert!(
                error.contains(&format!("spec.layout.spec.rows[0].spec.{field}")),
                "unexpected error for {field}: {error}"
            );
        }
    }

    #[test]
    fn v2_rows_default_absent_title_to_empty_string() {
        let mut json = valid_v2_resource();
        let grid = json["spec"]["layout"].clone();
        json["spec"]["layout"] = serde_json::json!({
            "kind": "RowsLayout",
            "spec": {"rows": [{
                "kind": "RowsLayoutRow",
                "spec": {"layout": grid}
            }]}
        });

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();

        assert_eq!(
            dashboard
                .layout
                .row(crate::dashboard::RowId::new(0))
                .unwrap()
                .title,
            ""
        );
    }

    #[test]
    fn v2_rows_default_absent_collapse_and_hide_header_to_false() {
        let mut json = valid_v2_resource();
        make_v2_panel_importable(&mut json);
        let grid = json["spec"]["layout"].clone();
        json["spec"]["layout"] = serde_json::json!({
            "kind": "RowsLayout",
            "spec": {"rows": [{
                "kind": "RowsLayoutRow",
                "spec": {"title": "Defaults", "layout": grid}
            }]}
        });

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();
        let row = dashboard
            .layout
            .row(crate::dashboard::RowId::new(0))
            .unwrap();

        assert!(!row.collapsed);
        assert!(!row.hidden_header);
        assert_eq!(dashboard.layout.visible_panel_indices(), vec![0]);
        assert_eq!(
            dashboard
                .layout
                .visible_items()
                .into_iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![
                crate::dashboard::DashboardItemId::Row(crate::dashboard::RowId::new(0)),
                crate::dashboard::DashboardItemId::Panel(0),
            ]
        );
    }

    #[test]
    fn v2_rows_accept_empty_deferred_fields() {
        for (field, value) in [
            ("variables", serde_json::json!([])),
            ("fillScreen", serde_json::json!(false)),
        ] {
            let dashboard =
                parse_grafana_dashboard(&v2_row_resource_with_field(field, value).to_string())
                    .unwrap();
            assert!(dashboard.layout.visible_panel_indices().is_empty());
        }
    }

    #[test]
    fn v2_rows_reject_malformed_rows_at_native_paths() {
        let cases = [
            (
                serde_json::json!({"kind": "RowsLayout", "spec": {"rows": {}}}),
                "spec.layout.spec.rows",
            ),
            (
                serde_json::json!({"kind": "RowsLayout", "spec": {"rows": [null]}}),
                "spec.layout.spec.rows[0]",
            ),
            (
                serde_json::json!({"kind": "RowsLayout", "spec": {"rows": [{"kind": "GridLayoutItem", "spec": {}}]}}),
                "spec.layout.spec.rows[0].kind",
            ),
            (
                serde_json::json!({"kind": "RowsLayout", "spec": {"rows": [{"kind": "RowsLayoutRow", "spec": []}]}}),
                "spec.layout.spec.rows[0].spec",
            ),
            (
                serde_json::json!({"kind": "RowsLayout", "spec": {"rows": [{"kind": "RowsLayoutRow", "spec": {"title": 7, "layout": {"kind": "GridLayout", "spec": {"items": []}}}}]}}),
                "spec.layout.spec.rows[0].spec.title",
            ),
            (
                serde_json::json!({"kind": "RowsLayout", "spec": {"rows": [{"kind": "RowsLayoutRow", "spec": {"title": "Row", "layout": []}}]}}),
                "spec.layout.spec.rows[0].spec.layout",
            ),
        ];

        for (layout, expected_path) in cases {
            let json = minimal_v2_with_layout(layout);
            let error = parse_grafana_dashboard(&json.to_string())
                .unwrap_err()
                .to_string();
            assert!(
                error.contains(expected_path),
                "expected {expected_path}, got {error}"
            );
        }
    }

    #[test]
    fn v2_rows_reject_non_boolean_options_at_native_paths() {
        for field in ["collapse", "hideHeader", "fillScreen"] {
            let error = parse_grafana_dashboard(
                &v2_row_resource_with_field(field, serde_json::json!("false")).to_string(),
            )
            .unwrap_err()
            .to_string();
            assert!(
                error.contains(&format!("spec.layout.spec.rows[0].spec.{field}")),
                "unexpected error for {field}: {error}"
            );
        }
    }

    #[test]
    fn v2_rows_reject_nested_unsupported_layouts_at_native_paths() {
        for kind in ["TabsLayout", "AutoGridLayout"] {
            let json =
                v2_row_resource_with_field("layout", serde_json::json!({"kind": kind, "spec": {}}));
            let error = parse_grafana_dashboard(&json.to_string())
                .unwrap_err()
                .to_string();
            assert!(error.contains(kind));
            assert!(
                error.contains("spec.layout.spec.rows[0].spec.layout.kind"),
                "unexpected error for {kind}: {error}"
            );
        }
    }

    #[test]
    fn rejects_invalid_v2_resource_kind() {
        for kind in [None, Some("Folder")] {
            let mut json = valid_v2_resource();
            if let Some(kind) = kind {
                json["kind"] = serde_json::json!(kind);
            } else {
                json.as_object_mut().unwrap().remove("kind");
            }
            assert!(
                parse_grafana_dashboard(&json.to_string())
                    .unwrap_err()
                    .to_string()
                    .contains("kind")
            );
        }
    }

    #[test]
    fn rejects_missing_or_non_object_v2_spec() {
        for spec in [None, Some(serde_json::json!([]))] {
            let mut json = valid_v2_resource();
            if let Some(spec) = spec {
                json["spec"] = spec;
            } else {
                json.as_object_mut().unwrap().remove("spec");
            }
            assert!(
                parse_grafana_dashboard(&json.to_string())
                    .unwrap_err()
                    .to_string()
                    .contains("spec")
            );
        }
    }

    #[test]
    fn rejects_malformed_v2_time_settings_fields() {
        let mut non_object_settings = valid_v2_resource();
        non_object_settings["spec"]["timeSettings"] = serde_json::json!("30s");
        let error = parse_grafana_dashboard(&non_object_settings.to_string())
            .unwrap_err()
            .to_string();
        assert!(error.contains("spec.timeSettings"));

        let mut non_string_refresh = valid_v2_resource();
        non_string_refresh["spec"]["timeSettings"]["autoRefresh"] = serde_json::json!(30);
        let error = parse_grafana_dashboard(&non_string_refresh.to_string())
            .unwrap_err()
            .to_string();
        assert!(error.contains("spec.timeSettings.autoRefresh"));
    }

    #[test]
    fn rejects_invalid_v2_nested_kinds() {
        let cases = [
            (
                "data",
                "spec.elements[\"panel-1\"].spec.data.kind",
                "NotQueryGroup",
            ),
            (
                "query",
                "spec.elements[\"panel-1\"].spec.data.spec.queries[0].kind",
                "NotPanelQuery",
            ),
            (
                "data_query",
                "spec.elements[\"panel-1\"].spec.data.spec.queries[0].spec.query.kind",
                "NotDataQuery",
            ),
            (
                "viz",
                "spec.elements[\"panel-1\"].spec.vizConfig.kind",
                "NotVizConfig",
            ),
        ];

        for (case, expected_path, replacement) in cases {
            let mut value: serde_json::Value = serde_json::from_str(include_str!(
                "../tests/fixtures/grafana/v2_compatibility.json"
            ))
            .unwrap();
            match case {
                "data" => {
                    value["spec"]["elements"]["panel-1"]["spec"]["data"]["kind"] =
                        replacement.into()
                }
                "query" => {
                    value["spec"]["elements"]["panel-1"]["spec"]["data"]["spec"]["queries"][0]["kind"] =
                        replacement.into()
                }
                "data_query" => {
                    value["spec"]["elements"]["panel-1"]["spec"]["data"]["spec"]["queries"][0]["spec"]
                        ["query"]["kind"] = replacement.into()
                }
                "viz" => {
                    value["spec"]["elements"]["panel-1"]["spec"]["vizConfig"]["kind"] =
                        replacement.into()
                }
                _ => unreachable!(),
            }
            let error = parse_grafana_dashboard(&value.to_string())
                .unwrap_err()
                .to_string();
            assert!(error.contains(replacement));
            assert!(error.contains(expected_path));
        }
    }

    #[test]
    fn v2_query_variable_falls_back_to_definition_with_its_native_path() {
        let mut json = valid_v2_resource();
        json["spec"]["variables"] = serde_json::json!([{
            "kind": "QueryVariable",
            "spec": {
                "name": "instance",
                "current": {"text": "node-1", "value": "node-1"},
                "query": {"kind": "DataQuery", "group": "prometheus", "version": "v0", "spec": {"query": "  "}},
                "definition": "  label_values(up, instance)  "
            }
        }]);

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();

        assert_eq!(
            dashboard.vars.get("instance").map(String::as_str),
            Some("node-1")
        );
        assert_eq!(dashboard.query_vars.len(), 1);
        assert_eq!(dashboard.query_vars[0].query, "label_values(up, instance)");
        assert_eq!(
            dashboard.query_vars[0].query_path,
            "spec.variables[0].spec.definition"
        );
    }

    #[test]
    fn rejects_malformed_v2_query_variable_wrappers_at_native_paths() {
        for (case, expected_path) in [
            ("missing_query", "spec.variables[0].spec.query"),
            ("wrong_query_type", "spec.variables[0].spec.query"),
            ("missing_kind", "spec.variables[0].spec.query.kind"),
            ("wrong_kind", "spec.variables[0].spec.query.kind"),
            ("wrong_kind_type", "spec.variables[0].spec.query.kind"),
            ("missing_group", "spec.variables[0].spec.query.group"),
            ("wrong_group_type", "spec.variables[0].spec.query.group"),
        ] {
            let mut json = valid_v2_resource();
            json["spec"]["variables"] = serde_json::json!([{
                "kind": "QueryVariable",
                "spec": {
                    "name": "instance",
                    "current": {"text": "node-1", "value": "node-1"},
                    "query": {
                        "kind": "DataQuery",
                        "group": "prometheus",
                        "spec": {"query": "label_values(up, instance)"}
                    },
                    "definition": "label_values(up, instance)"
                }
            }]);

            let variable_spec = json["spec"]["variables"][0]["spec"]
                .as_object_mut()
                .unwrap();
            match case {
                "missing_query" => {
                    variable_spec.remove("query");
                }
                "wrong_query_type" => {
                    variable_spec.insert("query".into(), serde_json::json!([]));
                }
                "missing_kind" => {
                    variable_spec["query"]
                        .as_object_mut()
                        .unwrap()
                        .remove("kind");
                }
                "wrong_kind" => variable_spec["query"]["kind"] = serde_json::json!("FutureQuery"),
                "wrong_kind_type" => variable_spec["query"]["kind"] = serde_json::json!(1),
                "missing_group" => {
                    variable_spec["query"]
                        .as_object_mut()
                        .unwrap()
                        .remove("group");
                }
                "wrong_group_type" => variable_spec["query"]["group"] = serde_json::json!(1),
                _ => unreachable!(),
            };

            let error = parse_grafana_dashboard(&json.to_string())
                .expect_err(case)
                .to_string();
            assert!(
                error.contains(expected_path),
                "{case}: expected `{expected_path}` in `{error}`"
            );
        }
    }

    #[test]
    fn rejects_missing_or_non_object_v2_query_variable_data_query_spec() {
        for case in ["missing_spec", "wrong_spec_type"] {
            let mut json = valid_v2_resource();
            json["spec"]["variables"] = serde_json::json!([{
                "kind": "QueryVariable",
                "spec": {
                    "name": "instance",
                    "current": {"text": "node-1", "value": "node-1"},
                    "query": {
                        "kind": "DataQuery",
                        "group": "prometheus",
                        "spec": {"query": "label_values(up, instance)"}
                    },
                    "definition": "label_values(up, instance)"
                }
            }]);
            let query = json["spec"]["variables"][0]["spec"]["query"]
                .as_object_mut()
                .unwrap();
            match case {
                "missing_spec" => {
                    query.remove("spec");
                }
                "wrong_spec_type" => {
                    query.insert("spec".into(), serde_json::json!([]));
                }
                _ => unreachable!(),
            }

            let error = parse_grafana_dashboard(&json.to_string())
                .expect_err(case)
                .to_string();
            assert!(
                error.contains("spec.variables[0].spec.query.spec"),
                "{case}: expected native DataQuery.spec path in `{error}`"
            );
        }
    }

    #[test]
    fn v2_supported_option_variables_import_object_current_values() {
        let mut json = valid_v2_resource();
        json["spec"]["variables"] = serde_json::json!([
            {"kind": "TextVariable", "spec": {"name": "text", "current": {"text": "one", "value": "one"}}},
            {"kind": "ConstantVariable", "spec": {"name": "constant", "current": {"text": "two", "value": "two"}}},
            {"kind": "DatasourceVariable", "spec": {"name": "datasource", "current": {"text": "three", "value": "three"}}},
            {"kind": "IntervalVariable", "spec": {"name": "interval", "current": {"text": "four", "value": "four"}}},
            {"kind": "CustomVariable", "spec": {"name": "custom", "current": {"text": "five", "value": "five"}}},
            {"kind": "GroupByVariable", "spec": {"name": "group_by", "current": {"text": "six", "value": "six"}}}
        ]);

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();

        for (name, value) in [
            ("text", "one"),
            ("constant", "two"),
            ("datasource", "three"),
            ("interval", "four"),
            ("custom", "five"),
            ("group_by", "six"),
        ] {
            assert_eq!(dashboard.vars.get(name).map(String::as_str), Some(value));
        }
    }

    #[test]
    fn v2_switch_variable_imports_direct_current_value() {
        let mut json = valid_v2_resource();
        json["spec"]["variables"] = serde_json::json!([{
            "kind": "SwitchVariable",
            "spec": {"name": "show_total", "current": "true"}
        }]);

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();

        assert_eq!(
            dashboard.vars.get("show_total").map(String::as_str),
            Some("true")
        );
    }

    #[test]
    fn v2_diagnostics_non_prometheus_query_variable_is_not_dynamic() {
        let mut json = valid_v2_resource();
        json["spec"]["variables"] = serde_json::json!([{
            "kind": "QueryVariable",
            "spec": {
                "name": "service",
                "current": {"text": "api", "value": "api"},
                "query": {"kind": "DataQuery", "group": "loki", "version": "v0", "spec": {"query": "label_values(service)"}}
            }
        }]);

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();

        assert_eq!(
            dashboard.vars.get("service").map(String::as_str),
            Some("api")
        );
        assert!(dashboard.query_vars.is_empty());
        assert_eq!(dashboard.diagnostics.len(), 1);
        assert_eq!(dashboard.diagnostics[0].code, "unsupported_datasource");
        assert_eq!(
            dashboard.diagnostics[0].path,
            "spec.variables[0].spec.query"
        );
    }

    #[test]
    fn v2_query_variable_uses_its_all_value_for_all_current_selection() {
        let mut json = valid_v2_resource();
        json["spec"]["variables"] = serde_json::json!([{
            "kind": "QueryVariable",
            "spec": {
                "name": "job",
                "current": {"text": "All", "value": "$__all"},
                "query": {"kind": "DataQuery", "group": "prometheus", "version": "v0", "spec": {"query": "label_values(up, job)"}},
                "allValue": "api|worker"
            }
        }]);

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();

        assert_eq!(
            dashboard.vars.get("job").map(String::as_str),
            Some("api|worker")
        );
        assert!(dashboard.query_vars.is_empty());
    }

    #[test]
    fn v2_custom_variable_all_selection_matches_classic_all_value_behavior() {
        let classic = parse_grafana_dashboard(
            r#"{
                "title": "Classic",
                "templating": {"list": [{
                    "name": "region",
                    "type": "custom",
                    "current": {"text": "All", "value": "$__all"},
                    "allValue": "eu-west-1|us-east-1"
                }]}
            }"#,
        )
        .unwrap();
        let mut json = valid_v2_resource();
        json["spec"]["variables"] = serde_json::json!([{
            "kind": "CustomVariable",
            "spec": {
                "name": "region",
                "current": {"text": "All", "value": "$__all"},
                "allValue": "eu-west-1|us-east-1"
            }
        }]);

        let v2 = parse_grafana_dashboard(&json.to_string()).unwrap();

        assert_eq!(
            v2.vars.get("region").map(String::as_str),
            Some("eu-west-1|us-east-1")
        );
        assert_eq!(v2.vars.get("region"), classic.vars.get("region"));
    }

    #[test]
    fn rejects_malformed_required_v2_panel_fields_at_native_paths() {
        let cases = [
            ("missing_title", "spec.elements[\"panel-1\"].spec.title"),
            ("wrong_title_type", "spec.elements[\"panel-1\"].spec.title"),
            (
                "missing_transformations",
                "spec.elements[\"panel-1\"].spec.data.spec.transformations",
            ),
            (
                "wrong_transformations_type",
                "spec.elements[\"panel-1\"].spec.data.spec.transformations",
            ),
            (
                "missing_hidden",
                "spec.elements[\"panel-1\"].spec.data.spec.queries[0].spec.hidden",
            ),
            (
                "wrong_hidden_type",
                "spec.elements[\"panel-1\"].spec.data.spec.queries[0].spec.hidden",
            ),
            (
                "missing_data_query_group",
                "spec.elements[\"panel-1\"].spec.data.spec.queries[0].spec.query.group",
            ),
            (
                "wrong_data_query_group_type",
                "spec.elements[\"panel-1\"].spec.data.spec.queries[0].spec.query.group",
            ),
            (
                "missing_data_query_spec",
                "spec.elements[\"panel-1\"].spec.data.spec.queries[0].spec.query.spec",
            ),
            (
                "wrong_data_query_spec_type",
                "spec.elements[\"panel-1\"].spec.data.spec.queries[0].spec.query.spec",
            ),
            (
                "missing_viz_group",
                "spec.elements[\"panel-1\"].spec.vizConfig.group",
            ),
            (
                "wrong_viz_group_type",
                "spec.elements[\"panel-1\"].spec.vizConfig.group",
            ),
            (
                "missing_viz_spec",
                "spec.elements[\"panel-1\"].spec.vizConfig.spec",
            ),
            (
                "wrong_viz_spec_type",
                "spec.elements[\"panel-1\"].spec.vizConfig.spec",
            ),
            (
                "missing_field_config",
                "spec.elements[\"panel-1\"].spec.vizConfig.spec.fieldConfig",
            ),
            (
                "wrong_field_config_type",
                "spec.elements[\"panel-1\"].spec.vizConfig.spec.fieldConfig",
            ),
            (
                "missing_defaults",
                "spec.elements[\"panel-1\"].spec.vizConfig.spec.fieldConfig.defaults",
            ),
            (
                "wrong_defaults_type",
                "spec.elements[\"panel-1\"].spec.vizConfig.spec.fieldConfig.defaults",
            ),
            (
                "missing_options",
                "spec.elements[\"panel-1\"].spec.vizConfig.spec.options",
            ),
            (
                "wrong_options_type",
                "spec.elements[\"panel-1\"].spec.vizConfig.spec.options",
            ),
        ];

        for (case, expected_path) in cases {
            let mut json: serde_json::Value = serde_json::from_str(include_str!(
                "../tests/fixtures/grafana/v2_compatibility.json"
            ))
            .unwrap();
            let panel = &mut json["spec"]["elements"]["panel-1"]["spec"];
            match case {
                "missing_title" => {
                    panel.as_object_mut().unwrap().remove("title");
                }
                "wrong_title_type" => panel["title"] = serde_json::json!(1),
                "missing_transformations" => {
                    panel["data"]["spec"]
                        .as_object_mut()
                        .unwrap()
                        .remove("transformations");
                }
                "wrong_transformations_type" => {
                    panel["data"]["spec"]["transformations"] = serde_json::json!({})
                }
                "missing_hidden" => {
                    panel["data"]["spec"]["queries"][0]["spec"]
                        .as_object_mut()
                        .unwrap()
                        .remove("hidden");
                }
                "wrong_hidden_type" => {
                    panel["data"]["spec"]["queries"][0]["spec"]["hidden"] =
                        serde_json::json!("false")
                }
                "missing_data_query_group" => {
                    panel["data"]["spec"]["queries"][0]["spec"]["query"]
                        .as_object_mut()
                        .unwrap()
                        .remove("group");
                }
                "wrong_data_query_group_type" => {
                    panel["data"]["spec"]["queries"][0]["spec"]["query"]["group"] =
                        serde_json::json!(1)
                }
                "missing_data_query_spec" => {
                    panel["data"]["spec"]["queries"][0]["spec"]["query"]
                        .as_object_mut()
                        .unwrap()
                        .remove("spec");
                }
                "wrong_data_query_spec_type" => {
                    panel["data"]["spec"]["queries"][0]["spec"]["query"]["spec"] =
                        serde_json::json!([])
                }
                "missing_viz_group" => {
                    panel["vizConfig"].as_object_mut().unwrap().remove("group");
                }
                "wrong_viz_group_type" => panel["vizConfig"]["group"] = serde_json::json!(1),
                "missing_viz_spec" => {
                    panel["vizConfig"].as_object_mut().unwrap().remove("spec");
                }
                "wrong_viz_spec_type" => panel["vizConfig"]["spec"] = serde_json::json!([]),
                "missing_field_config" => {
                    panel["vizConfig"]["spec"]
                        .as_object_mut()
                        .unwrap()
                        .remove("fieldConfig");
                }
                "wrong_field_config_type" => {
                    panel["vizConfig"]["spec"]["fieldConfig"] = serde_json::json!([])
                }
                "missing_defaults" => {
                    panel["vizConfig"]["spec"]["fieldConfig"]
                        .as_object_mut()
                        .unwrap()
                        .remove("defaults");
                }
                "wrong_defaults_type" => {
                    panel["vizConfig"]["spec"]["fieldConfig"]["defaults"] = serde_json::json!([])
                }
                "missing_options" => {
                    panel["vizConfig"]["spec"]
                        .as_object_mut()
                        .unwrap()
                        .remove("options");
                }
                "wrong_options_type" => {
                    panel["vizConfig"]["spec"]["options"] = serde_json::json!([])
                }
                _ => unreachable!(),
            };

            let error = parse_grafana_dashboard(&json.to_string())
                .expect_err(case)
                .to_string();
            assert!(
                error.contains(expected_path),
                "{case}: expected `{expected_path}` in `{error}`"
            );
        }
    }

    #[test]
    fn rejects_malformed_required_v2_panel_id_and_links_at_native_paths() {
        for (case, expected_path) in [
            ("missing_id", "spec.elements[\"panel-1\"].spec.id"),
            ("wrong_id_type", "spec.elements[\"panel-1\"].spec.id"),
            ("missing_links", "spec.elements[\"panel-1\"].spec.links"),
            ("wrong_links_type", "spec.elements[\"panel-1\"].spec.links"),
        ] {
            let mut json: serde_json::Value = serde_json::from_str(include_str!(
                "../tests/fixtures/grafana/v2_compatibility.json"
            ))
            .unwrap();
            let panel = &mut json["spec"]["elements"]["panel-1"]["spec"];
            match case {
                "missing_id" => {
                    panel.as_object_mut().unwrap().remove("id");
                }
                "wrong_id_type" => panel["id"] = serde_json::json!("1"),
                "missing_links" => {
                    panel.as_object_mut().unwrap().remove("links");
                }
                "wrong_links_type" => panel["links"] = serde_json::json!({}),
                _ => unreachable!(),
            }

            let error = parse_grafana_dashboard(&json.to_string())
                .expect_err(case)
                .to_string();
            assert!(
                error.contains(expected_path),
                "{case}: expected `{expected_path}` in `{error}`"
            );
        }
    }

    #[test]
    fn v2_diagnostics_unsupported_variable_kinds_are_skipped_with_native_paths() {
        let mut json = valid_v2_resource();
        json["spec"]["variables"] = serde_json::json!([
            {"kind": "AdhocVariable", "spec": {"name": "adhoc"}},
            {"kind": "FutureVariable", "spec": {"name": "future"}}
        ]);

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();

        assert!(dashboard.vars.is_empty());
        assert!(dashboard.query_vars.is_empty());
        assert_eq!(dashboard.diagnostics.len(), 2);
        assert_eq!(dashboard.diagnostics[0].code, "unsupported_variable");
        assert_eq!(dashboard.diagnostics[0].path, "spec.variables[0]");
        assert_eq!(dashboard.diagnostics[1].code, "unsupported_variable");
        assert_eq!(dashboard.diagnostics[1].path, "spec.variables[1]");
    }

    #[test]
    fn v2_diagnostics_unsupported_elements_are_skipped_before_deserialization() {
        for (kind, name) in [("LibraryPanel", "library-1"), ("FutureElement", "future-1")] {
            let mut json = valid_v2_resource();
            json["spec"]["elements"] = serde_json::json!({
                name: {"kind": kind, "spec": {"name": "not-a-panel"}}
            });
            json["spec"]["layout"]["spec"]["items"][0]["spec"]["element"]["name"] = name.into();

            let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();

            assert!(dashboard.queries.is_empty());
            assert_eq!(dashboard.skipped_panels, 1);
            assert_eq!(dashboard.diagnostics.len(), 1);
            assert_eq!(dashboard.diagnostics[0].code, "unsupported_element");
            assert_eq!(
                dashboard.diagnostics[0].path,
                format!("spec.elements[{name:?}]")
            );
        }
    }

    #[test]
    fn v2_diagnostics_mixed_datasources_retain_prometheus_expressions() {
        let mut json = valid_v2_resource();
        json["spec"]["elements"]["panel-1"]["spec"]["data"]["spec"]["queries"] = serde_json::json!([
            {
                "kind": "PanelQuery",
                "spec": {
                    "hidden": false,
                    "refId": "A",
                    "query": {"kind": "DataQuery", "group": "loki", "spec": {"expr": "{app=\"api\"}"}}
                }
            },
            {
                "kind": "PanelQuery",
                "spec": {
                    "hidden": false,
                    "refId": "B",
                    "query": {"kind": "DataQuery", "group": "prometheus", "spec": {"expr": "up"}}
                }
            }
        ]);

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();

        assert_eq!(dashboard.queries.len(), 1);
        assert_eq!(dashboard.queries[0].exprs, ["up"]);
        assert_eq!(dashboard.diagnostics.len(), 1);
        assert_eq!(dashboard.diagnostics[0].code, "unsupported_datasource");
        assert_eq!(
            dashboard.diagnostics[0].path,
            "spec.elements[\"panel-1\"].spec.data.spec.queries[0].spec.query"
        );
    }

    #[test]
    fn v2_all_unsupported_datasource_targets_increment_skipped_panels_once() {
        let mut json = valid_v2_resource();
        json["spec"]["elements"]["panel-1"]["spec"]["data"]["spec"]["queries"] = serde_json::json!([{
            "kind": "PanelQuery",
            "spec": {
                "hidden": false,
                "refId": "A",
                "query": {
                    "kind": "DataQuery",
                    "group": "loki",
                    "spec": {"expr": "{app=\"api\"}"}
                }
            }
        }]);

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();

        assert!(dashboard.queries.is_empty());
        assert_eq!(dashboard.skipped_panels, 1);
        assert_eq!(dashboard.diagnostics.len(), 1);
        assert_eq!(dashboard.diagnostics[0].code, "unsupported_datasource");
        assert_eq!(
            dashboard.diagnostics[0].path,
            "spec.elements[\"panel-1\"].spec.data.spec.queries[0].spec.query"
        );
    }

    #[test]
    fn v2_diagnostics_missing_visible_prometheus_expression() {
        let mut json = valid_v2_resource();
        json["spec"]["elements"]["panel-1"]["spec"]["data"]["spec"]["queries"] = serde_json::json!([{
            "kind": "PanelQuery",
            "spec": {
                "hidden": false,
                "refId": "A",
                "query": {"kind": "DataQuery", "group": "prometheus", "spec": {"expr": "  "}}
            }
        }]);

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();

        assert!(dashboard.queries.is_empty());
        assert_eq!(dashboard.diagnostics.len(), 1);
        assert_eq!(dashboard.diagnostics[0].code, "missing_query_expression");
        assert_eq!(
            dashboard.diagnostics[0].path,
            "spec.elements[\"panel-1\"].spec.data.spec.queries[0].spec.query.spec.expr"
        );
    }

    #[test]
    fn v2_diagnostics_transformations_use_the_native_path() {
        let mut json = valid_v2_resource();
        json["spec"]["elements"]["panel-1"]["spec"]["data"]["spec"]["transformations"] =
            serde_json::json!([{"kind": "reduce"}]);

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();

        assert_eq!(dashboard.diagnostics.len(), 1);
        assert_eq!(dashboard.diagnostics[0].code, "ignored_field");
        assert_eq!(
            dashboard.diagnostics[0].path,
            "spec.elements[\"panel-1\"].spec.data.spec.transformations"
        );
    }

    #[test]
    fn v2_diagnostics_unsupported_viz_groups_increment_skipped_panels() {
        let mut json = valid_v2_resource();
        json["spec"]["elements"]["panel-1"]["spec"]["vizConfig"]["group"] =
            serde_json::json!("piechart");

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();

        assert!(dashboard.queries.is_empty());
        assert_eq!(dashboard.skipped_panels, 1);
        assert_eq!(dashboard.diagnostics.len(), 1);
        assert_eq!(dashboard.diagnostics[0].code, "skipped_panel");
        assert_eq!(dashboard.diagnostics[0].path, "spec.elements[\"panel-1\"]");
    }

    #[test]
    fn v2_diagnostics_mappings_and_reduce_options_reuse_classic_messages() {
        let classic = parse_grafana_dashboard(
            r#"{
                "title": "Classic",
                "panels": [{
                    "type": "stat",
                    "title": "Panel",
                    "targets": [{"expr": "up"}],
                    "fieldConfig": {"defaults": {"mappings": [{"type": "value"}]}},
                    "options": {"reduceOptions": {"calcs": ["lastNotNull"]}}
                }]
            }"#,
        )
        .unwrap();
        let mut json = valid_v2_resource();
        json["spec"]["elements"]["panel-1"]["spec"]["vizConfig"]["spec"]["fieldConfig"]["defaults"]
            ["mappings"] = serde_json::json!([{"type": "value"}]);
        json["spec"]["elements"]["panel-1"]["spec"]["vizConfig"]["spec"]["options"]["reduceOptions"] =
            serde_json::json!({"calcs": ["lastNotNull"]});

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();

        assert_eq!(dashboard.diagnostics.len(), 2);
        assert_eq!(dashboard.diagnostics[0].code, "ignored_field");
        assert_eq!(
            dashboard.diagnostics[0].path,
            "spec.elements[\"panel-1\"].spec.vizConfig.spec.options.reduceOptions"
        );
        assert_eq!(
            dashboard.diagnostics[0].message,
            classic.diagnostics[0].message
        );
        assert_eq!(dashboard.diagnostics[1].code, "ignored_field");
        assert_eq!(
            dashboard.diagnostics[1].path,
            "spec.elements[\"panel-1\"].spec.vizConfig.spec.fieldConfig.defaults.mappings"
        );
        assert_eq!(
            dashboard.diagnostics[1].message,
            classic.diagnostics[1].message
        );
    }

    #[test]
    fn v2_diagnostics_variable_analysis_uses_v2_expression_paths() {
        let mut json = valid_v2_resource();
        json["spec"]["variables"] = serde_json::json!([{
            "kind": "CustomVariable",
            "spec": {"name": "job", "current": {"text": "api", "value": "api"}}
        }]);
        json["spec"]["elements"]["panel-1"]["spec"]["data"]["spec"]["queries"] = serde_json::json!([{
            "kind": "PanelQuery",
            "spec": {
                "hidden": false,
                "refId": "A",
                "query": {
                    "kind": "DataQuery",
                    "group": "prometheus",
                    "spec": {"expr": "up{job=~\"${job:regex}\", cluster=\"$cluster\"}"}
                }
            }
        }]);

        let dashboard = parse_grafana_dashboard(&json.to_string()).unwrap();
        let diagnostics = variable_diagnostics(&dashboard, &dashboard.vars);

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.path
                == "spec.elements[\"panel-1\"].spec.data.spec.queries[0].spec.query.spec.expr"
        }));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "unsupported_variable_modifier")
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "unresolved_variable")
        );
    }

    #[test]
    fn rejects_v2_grid_item_repeat_presence() {
        let mut json = valid_v2_resource();
        json["spec"]["layout"]["spec"]["items"][0]["spec"]["repeat"] = serde_json::Value::Null;
        assert!(
            parse_grafana_dashboard(&json.to_string())
                .unwrap_err()
                .to_string()
                .contains("spec.layout.spec.items[0].spec.repeat")
        );
    }

    #[test]
    fn rejects_wrong_v2_grid_item_kind() {
        let mut json = valid_v2_resource();
        json["spec"]["layout"]["spec"]["items"][0]["kind"] = serde_json::json!("RowsLayoutItem");
        assert!(
            parse_grafana_dashboard(&json.to_string())
                .unwrap_err()
                .to_string()
                .contains("spec.layout.spec.items[0].kind")
        );
    }

    #[test]
    fn rejects_wrong_v2_element_reference_kind() {
        let mut json = valid_v2_resource();
        json["spec"]["layout"]["spec"]["items"][0]["spec"]["element"]["kind"] =
            serde_json::json!("Panel");
        assert!(
            parse_grafana_dashboard(&json.to_string())
                .unwrap_err()
                .to_string()
                .contains("spec.layout.spec.items[0].spec.element.kind")
        );
    }

    #[test]
    fn rejects_invalid_v2_grid_coordinate() {
        for value in [
            None,
            Some(serde_json::json!("0")),
            Some(serde_json::json!(2147483648_u64)),
        ] {
            let mut json = valid_v2_resource();
            if let Some(value) = value {
                json["spec"]["layout"]["spec"]["items"][0]["spec"]["x"] = value;
            } else {
                json["spec"]["layout"]["spec"]["items"][0]["spec"]
                    .as_object_mut()
                    .unwrap()
                    .remove("x");
            }
            assert!(
                parse_grafana_dashboard(&json.to_string())
                    .unwrap_err()
                    .to_string()
                    .contains("spec.layout.spec.items[0].spec.x")
            );
        }
    }

    #[test]
    fn rejects_missing_v2_element_name() {
        let mut json = valid_v2_resource();
        json["spec"]["layout"]["spec"]["items"][0]["spec"]["element"]
            .as_object_mut()
            .unwrap()
            .remove("name");
        assert!(
            parse_grafana_dashboard(&json.to_string())
                .unwrap_err()
                .to_string()
                .contains("spec.layout.spec.items[0].spec.element.name")
        );
    }

    #[test]
    fn rejects_unresolved_v2_element_name() {
        let mut json = valid_v2_resource();
        json["spec"]["layout"]["spec"]["items"][0]["spec"]["element"]["name"] =
            serde_json::json!("absent");
        assert!(
            parse_grafana_dashboard(&json.to_string())
                .unwrap_err()
                .to_string()
                .contains("spec.layout.spec.items[0].spec.element.name")
        );
    }

    #[test]
    fn rejects_yaml_input() {
        let error = parse_grafana_dashboard(
            "apiVersion: dashboard.grafana.app/v2\\nkind: Dashboard\\nspec: {}\\n",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("parsing Grafana dashboard JSON"));
    }

    #[test]
    fn classic_import_runs_through_normalized_model_without_semantic_changes() {
        let dashboard = parse_grafana_dashboard(
            r#"{
          "title":"Classic parity",
          "refresh":"15s",
          "templating":{"list":[{
            "name":"job",
            "type":"query",
            "query":"label_values(up, job)",
            "current":{"text":"api","value":"api"},
            "regex":""
          }]},
          "panels":[{
            "type":"timeseries",
            "title":"Requests",
            "gridPos":{"x":1,"y":2,"w":12,"h":8},
            "targets":[
              {"expr":"hidden","hide":true},
              {"expr":"rate(requests_total{job=\"$job\"}[5m])","legendFormat":"{{instance}}","instant":false}
            ],
            "fieldConfig":{"defaults":{"unit":"reqps","decimals":1,"min":0,"max":100}},
            "options":{"reduceOptions":{"calcs":["lastNotNull"]}}
          }]
        }"#,
        )
        .unwrap();

        assert_eq!(dashboard.title, "Classic parity");
        assert_eq!(dashboard.refresh_rate_ms, Some(15_000));
        assert_eq!(dashboard.vars.get("job").map(String::as_str), Some("api"));
        assert_eq!(dashboard.query_vars[0].query, "label_values(up, job)");
        assert_eq!(dashboard.queries.len(), 1);
        let panel = &dashboard.queries[0];
        assert_eq!(panel.title, "Requests");
        assert_eq!(panel.exprs, ["rate(requests_total{job=\"$job\"}[5m])"]);
        assert_eq!(panel.expr_paths, ["panels[0].targets[1].expr"]);
        assert_eq!(panel.legends, [Some("{{instance}}".to_string())]);
        assert_eq!(panel.query_modes, [crate::app::QueryMode::Range]);
        assert_eq!((panel.grid.unwrap().x, panel.grid.unwrap().y), (1, 2));
        assert_eq!((panel.grid.unwrap().w, panel.grid.unwrap().h), (12, 8));
        assert_eq!(panel.display.unit.as_deref(), Some("reqps"));
        assert_eq!(panel.display.decimals, Some(1));
        assert_eq!((panel.min, panel.max), (Some(0.0), Some(100.0)));
        assert_eq!(dashboard.diagnostics[0].code, "ignored_field");
        assert_eq!(
            dashboard.diagnostics[0].path,
            "panels[0].options.reduceOptions"
        );
    }

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

        let dashboard = parse_grafana_dashboard(json).unwrap();

        assert_eq!(dashboard.title, "Test Dash");
        assert_eq!(
            dashboard.vars.get("job"),
            Some(&"node-exporter".to_string())
        );
        assert_eq!(dashboard.vars.get("instance"), Some(&"server1".to_string()));
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

        let out = parse_grafana_dashboard(json).unwrap();

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

        let out = parse_grafana_dashboard(json).unwrap();

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
