use serde_json::Value;

use super::ImportDiagnostic;

#[derive(Debug, Default)]
pub(super) struct Dashboard {
    pub(super) title: String,
    pub(super) refresh: Option<String>,
    pub(super) variables: Vec<Variable>,
    pub(super) layout: Vec<LayoutNode>,
    pub(super) skipped_panels: usize,
    pub(super) diagnostics: Vec<ImportDiagnostic>,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub(super) enum LayoutNode {
    Panel(Panel),
    Row(Row),
    Tabs(Tabs),
    AutoGrid(AutoGrid),
}

#[derive(Debug)]
pub(super) struct AutoGrid {
    pub(super) options: crate::dashboard::autogrid::AutoGridOptions,
    pub(super) items: Vec<AutoGridItem>,
}

#[derive(Debug)]
pub(super) struct Tabs {
    pub(super) tabs: Vec<Tab>,
}

#[derive(Debug)]
pub(super) struct Tab {
    pub(super) variables: Vec<Variable>,
    pub(super) title: String,
    #[allow(dead_code)]
    pub(super) source_path: String,
    pub(super) children: Vec<LayoutNode>,
}

#[derive(Debug)]
pub(super) struct Row {
    pub(super) variables: Vec<Variable>,
    pub(super) title: String,
    pub(super) collapsed: bool,
    pub(super) hidden_header: bool,
    #[allow(dead_code)]
    pub(super) source_path: String,
    pub(super) children: Vec<LayoutNode>,
}

#[derive(Debug)]
pub(super) struct Variable {
    pub(super) retained: Option<crate::dashboard::variables::Variable>,
    pub(super) name: String,
    pub(super) kind: Option<String>,
    pub(super) current: Option<VariableCurrent>,
    pub(super) query: Option<String>,
    pub(super) regex: Option<String>,
    pub(super) all_value: Option<String>,
    #[allow(dead_code)]
    pub(super) source_path: String,
    pub(super) query_path: Option<String>,
}

#[derive(Debug)]
pub(super) struct VariableCurrent {
    pub(super) text: Option<Value>,
    pub(super) value: Option<Value>,
}

#[derive(Debug)]
pub(super) struct Panel {
    pub(super) kind: String,
    pub(super) title: String,
    pub(super) source_path: String,
    pub(super) targets: Vec<Target>,
    pub(super) count_as_skipped_if_empty: bool,
    pub(super) grid: Option<GridPos>,
    pub(super) field_defaults: Option<FieldDefaults>,
    pub(super) reduce_options_path: Option<String>,
    #[allow(dead_code)]
    pub(super) transformations_path: Option<String>,
    /// Panel min interval (Classic `interval`, V2 `queryOptions.interval`).
    pub(super) min_interval: Option<MinInterval>,
    pub(super) max_data_points: Option<u32>,
}

/// A Grafana min interval as written, such as `30s`, `>1m`, or `$interval`.
#[derive(Debug)]
pub(super) struct MinInterval {
    pub(super) text: String,
    pub(super) path: String,
}

impl MinInterval {
    /// Keeps a non-empty interval; Grafana saves an unset one as `""`.
    pub(super) fn new(text: Option<String>, path: String) -> Option<Self> {
        let text = text?.trim().to_string();
        (!text.is_empty()).then_some(Self { text, path })
    }
}

/// Reads `maxDataPoints`, which Grafana saves as a number or a numeric string.
pub(super) fn max_data_points(value: Option<&Value>) -> Option<u32> {
    let points = match value? {
        Value::Number(number) => number.as_f64()?,
        Value::String(text) => text.trim().parse().ok()?,
        _ => return None,
    };
    (points >= 1.0).then(|| points.min(u32::MAX as f64) as u32)
}

#[derive(Debug)]
pub(super) struct Target {
    pub(super) expr: Option<String>,
    pub(super) expr_path: String,
    pub(super) legend_format: Option<String>,
    pub(super) instant: Option<bool>,
    pub(super) hidden: bool,
    /// Per-query min step (`targets[].interval`).
    pub(super) min_interval: Option<MinInterval>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GridPos {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) w: i32,
    pub(super) h: i32,
}

#[derive(Debug, Default)]
pub(super) struct FieldDefaults {
    pub(super) unit: Option<String>,
    pub(super) decimals: Option<usize>,
    pub(super) no_value: Option<String>,
    pub(super) min: Option<f64>,
    pub(super) max: Option<f64>,
    pub(super) thresholds: Option<Thresholds>,
    pub(super) custom: Option<GraphCustom>,
    pub(super) mappings_path: Option<String>,
}

#[derive(Debug, Default)]
pub(super) struct GraphCustom {
    pub(super) draw_style: Option<String>,
    pub(super) show_points: Option<String>,
    pub(super) fill_opacity: Option<u16>,
    pub(super) axis_placement: Option<String>,
    pub(super) line_interpolation: Option<String>,
    pub(super) stacking_mode: Option<String>,
    pub(super) axis_grid_show: Option<bool>,
    pub(super) thresholds_style_mode: Option<String>,
}

#[derive(Debug)]
pub(super) struct Thresholds {
    pub(super) mode: Option<String>,
    pub(super) steps: Vec<ThresholdStep>,
}

#[derive(Debug)]
pub(super) struct ThresholdStep {
    pub(super) value: Option<f64>,
    pub(super) color: Option<String>,
}

#[derive(Debug)]
pub(super) struct AutoGridItem {
    pub(super) behavior: crate::dashboard::autogrid::AutoGridBehavior,
    pub(super) panel: Panel,
    pub(super) fit_content: Option<bool>,
}
