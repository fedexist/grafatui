//! Intrinsic sizes and presentation shared by terminal and export renderers.
use crate::app::{PanelState, PanelType};
use ratatui::style::Color;

pub(crate) fn measure_panel_content(panel: &PanelState, outer_width: u16) -> Option<u64> {
    if panel.panel_type != PanelType::Table {
        return None;
    }
    if outer_width.saturating_sub(2) == 0 {
        return Some(2);
    }
    if let Some(error) = &panel.last_error {
        return Some(
            super::error_body::error_body_window(error, outer_width - 2, 0, 0)
                .total_lines
                .saturating_add(2),
        );
    }
    let rows = panel.series.iter().filter(|s| s.visible).count() as u64;
    Some(if rows == 0 { 3 } else { rows.saturating_add(4) })
}

pub(crate) struct TableRowContent<'a> {
    pub(crate) name: &'a str,
    pub(crate) value: String,
    pub(crate) color: Option<Color>,
    /// SVG uses distinct default colors for numeric and missing values.
    pub(crate) has_value: bool,
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
            has_value: s.value.is_some(),
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PanelBodyMetrics {
    pub(crate) total: u64,
    pub(crate) capacity: u16,
    pub(crate) document_offset: u64,
}

impl PanelBodyMetrics {
    pub(crate) fn max_local_offset(self) -> u64 {
        self.total
            .saturating_sub(self.document_offset)
            .saturating_sub(u64::from(self.capacity))
    }
}

pub(crate) fn panel_body_offset(
    total: u64,
    document_offset: u64,
    local_offset: u64,
    capacity: u16,
) -> u64 {
    document_offset
        .saturating_add(local_offset)
        .min(total.saturating_sub(u64::from(capacity)))
}

pub(crate) fn panel_body_metrics(
    panel: &PanelState,
    outer_width: u16,
    outer_height: u16,
    document_offset: u64,
) -> PanelBodyMetrics {
    let inner_width = outer_width.saturating_sub(2);
    let (total, capacity) = if let Some(error) = &panel.last_error {
        (
            super::error_body::error_body_window(error, inner_width, 0, 0).total_lines,
            outer_height.saturating_sub(2),
        )
    } else {
        (
            panel.series.iter().filter(|s| s.visible).count() as u64,
            outer_height.saturating_sub(4),
        )
    };
    PanelBodyMetrics {
        total,
        capacity: if inner_width == 0 { 0 } else { capacity },
        document_offset,
    }
}
