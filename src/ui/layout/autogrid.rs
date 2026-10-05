use crate::dashboard::autogrid::AutoGridOptions;

/// Unscrolled document coordinates; convert to Ratatui coordinates after clipping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LogicalRect {
    pub(crate) x: u16,
    pub(crate) y: u64,
    pub(crate) width: u16,
    pub(crate) height: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutoGridPanelRect {
    pub(crate) index: usize,
    pub(crate) rect: LogicalRect,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct AutoGridProjection {
    pub(crate) panels: Vec<AutoGridPanelRect>,
    pub(crate) content_height: u64,
}

pub(crate) fn project_auto_grid(
    area_x: u16,
    width: u16,
    origin_y: u64,
    options: &AutoGridOptions,
    panels: &[usize],
) -> AutoGridProjection {
    if width == 0 || panels.is_empty() {
        return AutoGridProjection::default();
    }
    let fitting = (u32::from(width) / options.min_column_width.max(1)).max(1) as usize;
    let columns = options.max_columns.max(1).min(panels.len()).min(fitting);
    let base_width = usize::from(width) / columns;
    let remainder = usize::from(width) % columns;
    let height = u64::from(options.row_height.max(1));
    let projected = panels
        .iter()
        .enumerate()
        .map(|(position, &index)| {
            let column = position % columns;
            let row = position / columns;
            AutoGridPanelRect {
                index,
                rect: LogicalRect {
                    x: area_x.saturating_add((column * base_width + column.min(remainder)) as u16),
                    y: origin_y.saturating_add((row as u64).saturating_mul(height)),
                    width: (base_width + usize::from(column < remainder)) as u16,
                    height,
                },
            }
        })
        .collect();
    AutoGridProjection {
        panels: projected,
        content_height: (panels.len().div_ceil(columns) as u64).saturating_mul(height),
    }
}
