use super::{
    JsonObject, model, optional_bool_from, parse_panel, require_array_from, require_expected_kind,
    require_object_from, require_string_from,
};
use crate::{
    dashboard::autogrid::AutoGridOptions,
    display_units::{LOGICAL_CELL_HEIGHT_PX, LOGICAL_CELL_WIDTH_PX},
    grafana::ImportDiagnostic,
};
use anyhow::{Result, anyhow, bail, ensure};
use serde_json::Value;

pub(super) fn parse_auto_grid_layout(
    layout: &JsonObject,
    elements: &JsonObject,
    path: &str,
    diagnostics: &mut Vec<ImportDiagnostic>,
) -> Result<Vec<model::LayoutNode>> {
    let spec_path = format!("{path}.spec");
    let spec = require_object_from(layout, "spec", &spec_path)?;
    let options = parse_options(spec, &spec_path, diagnostics)?;
    warn_unknown(
        spec,
        &[
            "items",
            "maxColumnCount",
            "columnWidthMode",
            "columnWidth",
            "rowHeightMode",
            "rowHeight",
            "fillScreen",
            "fitContent",
            "matchRowHeights",
            "minHeightMode",
            "minHeight",
            "maxHeightMode",
            "maxHeight",
        ],
        &spec_path,
        diagnostics,
    );
    let items_path = format!("{spec_path}.items");
    let items = require_array_from(spec, "items", &items_path)?;
    let mut panels = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("{items_path}[{index}]");
        let item = item.as_object().ok_or_else(|| {
            anyhow!("invalid Grafana V2 AutoGrid item at {item_path}: expected an object")
        })?;
        require_expected_kind(item, &item_path, "AutoGridLayoutItem")?;
        let item_spec_path = format!("{item_path}.spec");
        let item_spec = require_object_from(item, "spec", &item_spec_path)?;
        for field in ["repeat", "conditionalRendering"] {
            ensure!(
                !item_spec.contains_key(field),
                "unsupported Grafana V2 AutoGrid setting at {item_spec_path}.{field}"
            );
        }
        reject_enabled(item_spec, "fitContent", &item_spec_path)?;
        warn_unknown(
            item_spec,
            &["element", "repeat", "conditionalRendering", "fitContent"],
            &item_spec_path,
            diagnostics,
        );
        let reference_path = format!("{item_spec_path}.element");
        let reference = require_object_from(item_spec, "element", &reference_path)?;
        require_expected_kind(reference, &reference_path, "ElementReference")?;
        let name = require_string_from(reference, "name", &format!("{reference_path}.name"))?;
        let element = elements.get(name).ok_or_else(|| {
            anyhow!("unresolved Grafana V2 element reference `{name}` at {reference_path}.name")
        })?;
        let element_path = format!("spec.elements[{name:?}]");
        if let Some(panel) = parse_panel(element, &element_path, None, diagnostics)? {
            panels.push(panel);
        }
    }
    Ok(vec![model::LayoutNode::AutoGrid(model::AutoGrid {
        options,
        panels,
    })])
}

fn parse_options(
    spec: &JsonObject,
    path: &str,
    diagnostics: &mut Vec<ImportDiagnostic>,
) -> Result<AutoGridOptions> {
    let max_columns = match optional_positive_number(spec, "maxColumnCount", path)? {
        None => 3,
        Some(value) => {
            ensure!(
                value >= 1.0 && value < usize::MAX as f64,
                "invalid Grafana V2 AutoGrid column count at {path}.maxColumnCount: expected a positive column count of at least 1 within platform limits"
            );
            if value.fract() != 0.0 {
                diagnostics.push(ImportDiagnostic::new(
                    "autogrid_column_limit_rounded",
                    format!("{path}.maxColumnCount"),
                    format!(
                        "fractional AutoGrid column limit {value} rounded down to {}",
                        value.floor()
                    ),
                ));
            }
            value.floor() as usize
        }
    };
    let column_width = optional_pixels(spec, "columnWidth", path, LOGICAL_CELL_WIDTH_PX)?;
    let row_height = optional_pixels(spec, "rowHeight", path, LOGICAL_CELL_HEIGHT_PX)?;
    let width_mode = mode(
        spec,
        "columnWidthMode",
        path,
        "standard",
        &["narrow", "standard", "wide", "custom"],
    )?;
    let height_mode = mode(
        spec,
        "rowHeightMode",
        path,
        "standard",
        &["short", "standard", "tall", "custom"],
    )?;
    let min_column_width = match width_mode {
        "narrow" => 192_u32.div_ceil(LOGICAL_CELL_WIDTH_PX),
        "wide" => 768_u32.div_ceil(LOGICAL_CELL_WIDTH_PX),
        "custom" => column_width.ok_or_else(|| anyhow!(
            "invalid Grafana V2 AutoGrid custom width at {path}.columnWidth: missing required positive number"
        ))?,
        _ => 448_u32.div_ceil(LOGICAL_CELL_WIDTH_PX),
    };
    let row_height = match height_mode {
        "short" => 168_u32.div_ceil(LOGICAL_CELL_HEIGHT_PX),
        "tall" => 512_u32.div_ceil(LOGICAL_CELL_HEIGHT_PX),
        "custom" => row_height.ok_or_else(|| anyhow!(
            "invalid Grafana V2 AutoGrid custom height at {path}.rowHeight: missing required positive number"
        ))?,
        _ => 320_u32.div_ceil(LOGICAL_CELL_HEIGHT_PX),
    };
    let fill_screen = optional_bool_from(spec, "fillScreen", path)?;
    reject_enabled(spec, "fitContent", path)?;
    optional_bool_from(spec, "matchRowHeights", path)?;
    validate_height_bounds(spec, path)?;
    Ok(AutoGridOptions {
        fill_screen,
        max_columns,
        min_column_width,
        row_height,
    })
}

fn validate_height_bounds(spec: &JsonObject, path: &str) -> Result<()> {
    let min = optional_pixels(spec, "minHeight", path, LOGICAL_CELL_HEIGHT_PX)?;
    let max = optional_pixels(spec, "maxHeight", path, LOGICAL_CELL_HEIGHT_PX)?;
    if spec.contains_key("minHeightMode") {
        mode(
            spec,
            "minHeightMode",
            path,
            "none",
            &["none", "short", "standard", "tall", "custom"],
        )?;
        bail!("unsupported Grafana V2 AutoGrid content height bound at {path}.minHeightMode");
    }
    if spec.contains_key("maxHeightMode") {
        let value = mode(
            spec,
            "maxHeightMode",
            path,
            "unlimited",
            &["unlimited", "short", "standard", "tall", "custom"],
        )?;
        ensure!(
            value == "unlimited",
            "unsupported Grafana V2 AutoGrid content height bound at {path}.maxHeightMode"
        );
    }
    ensure!(
        min.is_none(),
        "unsupported Grafana V2 AutoGrid content height bound at {path}.minHeight"
    );
    ensure!(
        max.is_none(),
        "unsupported Grafana V2 AutoGrid content height bound at {path}.maxHeight"
    );
    Ok(())
}

fn mode<'a>(
    spec: &'a JsonObject,
    field: &str,
    path: &str,
    default: &'a str,
    values: &[&str],
) -> Result<&'a str> {
    let value = match spec.get(field) {
        None => return Ok(default),
        Some(Value::String(value)) => value.as_str(),
        Some(_) => bail!("invalid Grafana V2 AutoGrid mode at {path}.{field}: expected a string"),
    };
    ensure!(
        values.contains(&value),
        "invalid Grafana V2 AutoGrid mode `{value}` at {path}.{field}"
    );
    Ok(value)
}

fn optional_positive_number(spec: &JsonObject, field: &str, path: &str) -> Result<Option<f64>> {
    let Some(raw) = spec.get(field) else {
        return Ok(None);
    };
    let value = raw.as_f64().filter(|value| value.is_finite() && *value > 0.0).ok_or_else(|| {
        anyhow!("invalid Grafana V2 AutoGrid size at {path}.{field}: expected a finite positive number")
    })?;
    Ok(Some(value))
}

fn optional_pixels(
    spec: &JsonObject,
    field: &str,
    path: &str,
    pixels_per_cell: u32,
) -> Result<Option<u32>> {
    optional_positive_number(spec, field, path)?.map(|pixels| {
        let cells = (pixels / f64::from(pixels_per_cell)).ceil();
        ensure!(cells <= f64::from(u32::MAX),
            "invalid Grafana V2 AutoGrid size at {path}.{field}: exceeds supported document dimensions");
        Ok(cells as u32)
    }).transpose()
}

fn reject_enabled(spec: &JsonObject, field: &str, path: &str) -> Result<()> {
    ensure!(
        !optional_bool_from(spec, field, path)?,
        "unsupported Grafana V2 AutoGrid setting at {path}.{field}"
    );
    Ok(())
}

fn warn_unknown(
    spec: &JsonObject,
    fields: &[&str],
    path: &str,
    diagnostics: &mut Vec<ImportDiagnostic>,
) {
    for field in spec
        .keys()
        .filter(|field| !fields.contains(&field.as_str()))
    {
        diagnostics.push(ImportDiagnostic::new(
            "unsupported_autogrid_setting",
            format!("{path}.{field}"),
            "unsupported AutoGrid setting ignored",
        ));
    }
}
