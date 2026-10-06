use crate::dashboard::autogrid::{AutoGridItem, AutoGridOptions};

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
    pub(crate) content_fit: bool,
    pub(crate) body_scroll: bool,
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
    items: &[AutoGridItem],
    measure: impl Fn(usize, u16) -> Option<u64>,
) -> AutoGridProjection {
    if width == 0 || items.is_empty() {
        return AutoGridProjection::default();
    }
    let fitting = (u32::from(width) / options.min_column_width.max(1)).max(1) as usize;
    let columns = options.max_columns.max(1).min(items.len()).min(fitting);
    let base_width = usize::from(width) / columns;
    let remainder = usize::from(width) % columns;
    let baseline = u64::from(options.row_height.max(1));
    let minimum = options.min_height.map(u64::from).unwrap_or(baseline);
    let cap = options
        .max_height
        .map(|h| u64::from(h).max(minimum))
        .unwrap_or(u64::MAX);
    let requested_fit =
        options.fit_content || items.iter().any(|item| item.fit_content == Some(true));
    let floor = if requested_fit && !options.fill_screen {
        baseline.min(minimum)
    } else {
        baseline
    };
    let mut tracks = vec![floor; items.len().div_ceil(columns)];
    let mut panels = Vec::with_capacity(items.len());
    for (position, item) in items.iter().enumerate() {
        let column = position % columns;
        let outer_width = (base_width + usize::from(column < remainder)) as u16;
        let natural = item
            .fit_content
            .unwrap_or(options.fit_content)
            .then(|| measure(item.index, outer_width))
            .flatten();
        let height = natural
            .map(|height| height.max(minimum).min(cap))
            .unwrap_or(baseline);
        tracks[position / columns] = tracks[position / columns].max(height);
        panels.push(AutoGridPanelRect {
            index: item.index,
            content_fit: natural.is_some(),
            body_scroll: natural.is_some() && options.max_height.is_some(),
            rect: LogicalRect {
                x: area_x.saturating_add((column * base_width + column.min(remainder)) as u16),
                y: 0,
                width: outer_width,
                height,
            },
        });
    }
    let natural_extent = tracks
        .iter()
        .fold(0_u64, |total, height| total.saturating_add(*height));
    let extra = if options.fill_screen {
        u64::from(allocated_height).saturating_sub(natural_extent)
    } else {
        0
    };
    let row_count = tracks.len() as u64;
    let mut cursor = origin_y;
    for (row, track) in tracks.iter_mut().enumerate() {
        *track =
            track.saturating_add(extra / row_count + u64::from((row as u64) < extra % row_count));
        for panel in panels.iter_mut().skip(row * columns).take(columns) {
            panel.rect.y = cursor;
            if options.match_row_heights {
                panel.rect.height = if panel.content_fit {
                    (*track).min(cap)
                } else {
                    *track
                };
            }
        }
        cursor = cursor.saturating_add(*track);
    }
    AutoGridProjection {
        panels,
        content_height: natural_extent.saturating_add(extra),
    }
}
