use serde_json::Value;

use super::ImportDiagnostic;

#[derive(Debug, Default)]
pub(super) struct Dashboard {
    pub(super) title: String,
    pub(super) refresh: Option<String>,
    pub(super) variables: Vec<Variable>,
    pub(super) panels: Vec<Panel>,
    pub(super) skipped_panels: usize,
    pub(super) diagnostics: Vec<ImportDiagnostic>,
}

#[derive(Debug)]
pub(super) struct Variable {
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
    pub(super) grid: Option<GridPos>,
    pub(super) field_defaults: Option<FieldDefaults>,
    pub(super) reduce_options_path: Option<String>,
    #[allow(dead_code)]
    pub(super) transformations_path: Option<String>,
}

#[derive(Debug)]
pub(super) struct Target {
    pub(super) expr: Option<String>,
    pub(super) expr_path: String,
    pub(super) legend_format: Option<String>,
    pub(super) instant: Option<bool>,
    pub(super) hidden: bool,
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
