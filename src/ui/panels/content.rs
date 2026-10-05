//! Intrinsic sizes and presentation shared by terminal and export renderers.
use crate::app::{PanelState, PanelType};
use ratatui::style::Color;

pub(crate) fn measure_panel_content(panel: &PanelState, outer_width: u16) -> Option<u64> {
    if panel.last_error.is_some() || panel.panel_type != PanelType::Table {
        return None;
    }
    if outer_width.saturating_sub(2) == 0 {
        return Some(2);
    }
    let rows = panel.series.iter().filter(|s| s.visible).count() as u64;
    Some(if rows == 0 { 3 } else { rows.saturating_add(4) })
}

pub(crate) struct TableRowContent<'a> {
    pub(crate) name: &'a str,
    pub(crate) value: String,
    pub(crate) color: Option<Color>,
}

pub(crate) fn prepare_table_rows(panel: &PanelState) -> Vec<TableRowContent<'_>> {
    panel
        .series
        .iter()
        .filter(|s| s.visible)
        .map(|s| TableRowContent {
            name: &s.name,
            value: panel.display.format_value(s.value),
            color: s.value.and_then(|value| panel.get_color_for_value(value)),
        })
        .collect()
}
