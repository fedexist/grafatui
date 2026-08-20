use anyhow::Result;

use super::{DashboardImport, GridPos, ImportDiagnostic, QueryPanel, TemplateQueryVar, model};

pub(super) fn finish(dashboard: model::Dashboard) -> Result<DashboardImport> {
    let mut out = DashboardImport {
        title: dashboard.title,
        refresh_rate_ms: dashboard.refresh.as_deref().and_then(parse_refresh_rate_ms),
        diagnostics: dashboard.diagnostics,
        ..DashboardImport::default()
    };
    import_variables(&mut out, dashboard.variables);
    import_panels(&mut out, dashboard.panels)?;
    Ok(out)
}

fn import_variables(out: &mut DashboardImport, variables: Vec<model::Variable>) {
    for variable in variables {
        let value = variable
            .current
            .as_ref()
            .and_then(|current| current.value.as_ref())
            .or(variable
                .current
                .as_ref()
                .and_then(|current| current.text.as_ref()));
        if let Some(value) = value {
            let mut value = match value {
                serde_json::Value::String(value) => value.clone(),
                serde_json::Value::Array(values) => values
                    .iter()
                    .find_map(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
                serde_json::Value::Number(value) => value.to_string(),
                _ => String::new(),
            };
            if value == "$__all" {
                value = variable
                    .all_value
                    .clone()
                    .unwrap_or_else(|| ".*".to_string());
            }
            if !value.is_empty() {
                out.vars.insert(variable.name.clone(), value);
            }
        }

        if variable.kind.as_deref() == Some("query")
            && !current_is_all(variable.current.as_ref())
            && let (Some(query), Some(query_path)) = (variable.query, variable.query_path)
        {
            out.query_vars.push(TemplateQueryVar {
                name: variable.name,
                query,
                regex: variable.regex.filter(|regex| !regex.trim().is_empty()),
                query_path,
            });
        }
    }
}

fn current_is_all(current: Option<&model::VariableCurrent>) -> bool {
    current.is_some_and(|current| {
        value_is_all(current.value.as_ref()) || value_is_all(current.text.as_ref())
    })
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

fn import_panels(out: &mut DashboardImport, panels: Vec<model::Panel>) -> Result<()> {
    for panel in panels {
        let panel_type = match panel.kind.as_str() {
            "graph" | "timeseries" => crate::app::PanelType::Graph,
            "stat" => crate::app::PanelType::Stat,
            "gauge" => crate::app::PanelType::Gauge,
            "bargauge" => crate::app::PanelType::BarGauge,
            "table" => crate::app::PanelType::Table,
            "heatmap" => crate::app::PanelType::Heatmap,
            _ => crate::app::PanelType::Unknown,
        };

        if panel_type == crate::app::PanelType::Unknown {
            if !panel.kind.is_empty() && panel.kind != "row" {
                out.skipped_panels += 1;
                out.diagnostics.push(ImportDiagnostic::new(
                    "skipped_panel",
                    panel.source_path,
                    format!(
                        "unsupported panel type `{}` skipped for panel `{}`",
                        panel.kind, panel.title
                    ),
                ));
            }
            continue;
        }

        let mut exprs = Vec::new();
        let mut expr_paths = Vec::new();
        let mut legends = Vec::new();
        let mut query_modes = Vec::new();
        for target in panel.targets {
            if target.hidden {
                continue;
            }
            if let Some(expr) = target.expr {
                exprs.push(expr);
                expr_paths.push(target.expr_path);
                legends.push(target.legend_format);
                query_modes.push(query_mode_for_target(target.instant, panel_type));
            }
        }

        let mut thresholds = None;
        let mut min = None;
        let mut max = None;
        let mut autogrid = None;
        let mut display = crate::ui::DisplayFormat::default();
        let mut graph_options = crate::app::GraphOptions::default();

        if let Some(path) = panel.transformations_path {
            out.diagnostics.push(ImportDiagnostic::new(
                "ignored_field",
                path,
                "`transformations` are not supported yet; queries will run without Grafana transformations",
            ));
        }

        if let Some(path) = panel.reduce_options_path {
            out.diagnostics.push(ImportDiagnostic::new(
                "ignored_field",
                path,
                "`options.reduceOptions` is not supported yet; Grafatui will use default value selection",
            ));
        }

        if let Some(defaults) = panel.field_defaults {
            if let Some(path) = defaults.mappings_path {
                out.diagnostics.push(ImportDiagnostic::new(
                    "ignored_field",
                    path,
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
            thresholds = thresholds_from_model(defaults.thresholds, defaults.custom.as_ref());
        }

        if !exprs.is_empty() {
            let options = match panel_type {
                crate::app::PanelType::Graph => crate::app::PanelOptions::Graph(graph_options),
                _ => crate::app::PanelOptions::None,
            };
            out.queries.push(QueryPanel {
                title: panel.title,
                exprs,
                expr_paths,
                legends,
                query_modes,
                grid: panel.grid.map(|grid| GridPos {
                    x: grid.x,
                    y: grid.y,
                    w: grid.w,
                    h: grid.h,
                }),
                panel_type,
                thresholds,
                min,
                max,
                autogrid,
                display,
                options,
            });
        }
    }
    Ok(())
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

fn graph_options_from_custom(custom: Option<&model::GraphCustom>) -> crate::app::GraphOptions {
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
        stacking: parse_graph_stacking_mode(custom.stacking_mode.as_deref()),
    }
}

fn thresholds_from_model(
    thresholds: Option<model::Thresholds>,
    custom: Option<&model::GraphCustom>,
) -> Option<crate::app::Thresholds> {
    let thresholds = thresholds?;
    let mode = match thresholds.mode.as_deref() {
        Some("percentage") => crate::app::ThresholdMode::Percentage,
        _ => crate::app::ThresholdMode::Absolute,
    };
    let mut steps: Vec<_> = thresholds
        .steps
        .into_iter()
        .map(|step| {
            let color = step.color.unwrap_or_else(|| "green".to_string());
            crate::app::ThresholdStep {
                value: step.value,
                color: crate::theme::parse_grafana_color(&color),
            }
        })
        .collect();
    steps.sort_by(|a, b| {
        let a_value = a.value.unwrap_or(f64::NEG_INFINITY);
        let b_value = b.value.unwrap_or(f64::NEG_INFINITY);
        a_value
            .partial_cmp(&b_value)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    (!steps.is_empty()).then(|| crate::app::Thresholds {
        mode,
        steps,
        style: Some(
            custom
                .and_then(|custom| custom.thresholds_style_mode.clone())
                .unwrap_or_else(|| "line".to_string()),
        ),
    })
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

fn parse_refresh_rate_ms(refresh: &str) -> Option<u64> {
    let refresh = refresh.trim();
    if refresh.is_empty()
        || refresh.eq_ignore_ascii_case("false")
        || refresh.eq_ignore_ascii_case("off")
    {
        return None;
    }
    let duration = humantime::parse_duration(refresh).ok()?;
    u64::try_from(duration.as_millis()).ok()
}
