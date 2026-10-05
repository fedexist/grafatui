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
    allocated_height: u16,
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
    let rows = panels.len().div_ceil(columns) as u64;
    let baseline_extent = rows.saturating_mul(u64::from(options.row_height.max(1)));
    let content_height = if options.fill_screen {
        baseline_extent.max(u64::from(allocated_height))
    } else {
        baseline_extent
    };
    let base_height = content_height / rows;
    let height_remainder = content_height % rows;
    let projected = panels
        .iter()
        .enumerate()
        .map(|(position, &index)| {
            let column = position % columns;
            let row = (position / columns) as u64;
            AutoGridPanelRect {
                index,
                rect: LogicalRect {
                    x: area_x.saturating_add((column * base_width + column.min(remainder)) as u16),
                    y: origin_y.saturating_add(
                        row.saturating_mul(base_height)
                            .saturating_add(row.min(height_remainder)),
                    ),
                    width: (base_width + usize::from(column < remainder)) as u16,
                    height: base_height + u64::from(row < height_remainder),
                },
            }
        })
        .collect();
    AutoGridProjection {
        panels: projected,
        content_height,
    }
}
