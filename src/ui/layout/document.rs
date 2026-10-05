//! Project nested dashboard content in document space, then clip it to the viewport.
use super::{
    DashboardRect, DashboardRectKind,
    autogrid::{LogicalRect, project_auto_grid},
};
use crate::{
    app::AppState,
    dashboard::{DashboardItemId, DashboardLayoutItem},
};
use ratatui::layout::{Constraint, Layout, Rect};

pub(super) struct ProjectedDashboardRect {
    pub(super) id: DashboardItemId,
    pub(super) rect: LogicalRect,
    pub(super) disclosure_width: Option<u16>,
    pub(super) kind: DashboardRectKind,
}

pub(super) fn project_dashboard(area: Rect, app: &AppState) -> (Vec<ProjectedDashboardRect>, u16) {
    let cell_h = (area.height / 24).max(3);
    let mut projected = Vec::new();
    project_layout_items(&app.layout.items, 0, area, 0, cell_h, app, &mut projected);
    (projected, cell_h)
}

fn project_layout_items(
    items: &[DashboardLayoutItem],
    depth: usize,
    area: Rect,
    mut cursor_y: u64,
    cell_h: u16,
    app: &AppState,
    output: &mut Vec<ProjectedDashboardRect>,
) -> u64 {
    let mut panels = Vec::new();
    for item in items {
        if let DashboardLayoutItem::Panel(index) = item {
            panels.push(*index);
            continue;
        }
        if !panels.is_empty() {
            cursor_y = project_panel_group(area, cursor_y, cell_h, app, &panels, output);
            panels.clear();
        }
        match item {
            DashboardLayoutItem::Panel(_) => unreachable!(),
            DashboardLayoutItem::AutoGrid(group) => {
                let projected =
                    project_auto_grid(area.x, area.width, cursor_y, &group.options, &group.panels);
                output.extend(
                    projected
                        .panels
                        .into_iter()
                        .map(|panel| panel_rect(panel.index, panel.rect)),
                );
                cursor_y = cursor_y.saturating_add(projected.content_height);
            }
            DashboardLayoutItem::Row(row) if row.hidden_header => {
                cursor_y =
                    project_layout_items(&row.children, depth, area, cursor_y, cell_h, app, output);
            }
            DashboardLayoutItem::Row(row) => {
                output.push(ProjectedDashboardRect {
                    id: DashboardItemId::Row(row.id),
                    rect: LogicalRect {
                        x: area.x,
                        y: cursor_y,
                        width: area.width,
                        height: 1,
                    },
                    disclosure_width: Some(area.width.min(1)),
                    kind: DashboardRectKind::Row {
                        row_id: row.id,
                        depth,
                        collapsed: row.collapsed,
                    },
                });
                cursor_y = cursor_y.saturating_add(1);
                if !row.collapsed {
                    cursor_y = project_layout_items(
                        &row.children,
                        depth + 1,
                        area,
                        cursor_y,
                        cell_h,
                        app,
                        output,
                    );
                }
            }
            DashboardLayoutItem::Tabs(group) => {
                output.push(ProjectedDashboardRect {
                    id: DashboardItemId::Tabs(group.id),
                    rect: LogicalRect {
                        x: area.x,
                        y: cursor_y,
                        width: area.width,
                        height: 1,
                    },
                    disclosure_width: None,
                    kind: DashboardRectKind::Tabs {
                        group_id: group.id,
                        depth,
                    },
                });
                cursor_y = cursor_y.saturating_add(1);
                if let Some(tab) = group.active.and_then(|index| group.tabs.get(index)) {
                    if tab.children.is_empty() {
                        output.push(ProjectedDashboardRect {
                            id: DashboardItemId::Tabs(group.id),
                            rect: LogicalRect {
                                x: area.x,
                                y: cursor_y,
                                width: area.width,
                                height: 1,
                            },
                            disclosure_width: None,
                            kind: DashboardRectKind::TabEmpty { group_id: group.id },
                        });
                        cursor_y = cursor_y.saturating_add(1);
                    } else {
                        cursor_y = project_layout_items(
                            &tab.children,
                            depth + 1,
                            area,
                            cursor_y,
                            cell_h,
                            app,
                            output,
                        );
                    }
                }
            }
        }
    }
    if !panels.is_empty() {
        cursor_y = project_panel_group(area, cursor_y, cell_h, app, &panels, output);
    }
    cursor_y
}

fn project_panel_group(
    area: Rect,
    origin_y: u64,
    cell_h: u16,
    app: &AppState,
    panel_indices: &[usize],
    output: &mut Vec<ProjectedDashboardRect>,
) -> u64 {
    let has_grid = panel_indices
        .iter()
        .any(|&index| app.panels.get(index).is_some_and(|p| p.grid.is_some()));
    if !has_grid {
        return project_flow_group(area, origin_y, panel_indices, output);
    }
    let cell_w = u64::from((area.width / 24).max(1));
    let cell_h = u64::from(cell_h);
    let mut grid_height = 0;
    let mut extras = Vec::new();
    for &index in panel_indices {
        let Some(panel) = app.panels.get(index) else {
            continue;
        };
        let Some(grid) = panel.grid else {
            extras.push(index);
            continue;
        };
        if grid.x < 0 || grid.y < 0 || grid.w <= 0 || grid.h <= 0 {
            continue;
        }
        let x_offset = (grid.x as u64).saturating_mul(cell_w);
        let y_offset = (grid.y as u64).saturating_mul(cell_h);
        let height = (grid.h as u64).saturating_mul(cell_h);
        grid_height = grid_height.max(y_offset.saturating_add(height));
        if x_offset >= u64::from(area.width) {
            continue;
        }
        let width = (grid.w as u64)
            .saturating_mul(cell_w)
            .min(u64::from(area.width) - x_offset) as u16;
        if width >= 8 && height >= 4 {
            output.push(panel_rect(
                index,
                LogicalRect {
                    x: area.x.saturating_add(x_offset as u16),
                    y: origin_y.saturating_add(y_offset),
                    width,
                    height,
                },
            ));
        }
    }
    project_flow_group(area, origin_y.saturating_add(grid_height), &extras, output)
}

fn project_flow_group(
    area: Rect,
    origin_y: u64,
    panels: &[usize],
    output: &mut Vec<ProjectedDashboardRect>,
) -> u64 {
    // Only horizontal geometry comes from Ratatui; document height is independent.
    let columns = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(Rect::new(area.x, 0, area.width, 1));
    for (position, &index) in panels.iter().enumerate() {
        let column = columns[position % 2];
        output.push(panel_rect(
            index,
            LogicalRect {
                x: column.x,
                y: origin_y.saturating_add((position as u64 / 2).saturating_mul(12)),
                width: column.width,
                height: 12,
            },
        ));
    }
    origin_y.saturating_add((panels.len().div_ceil(2) as u64).saturating_mul(12))
}

fn panel_rect(index: usize, rect: LogicalRect) -> ProjectedDashboardRect {
    ProjectedDashboardRect {
        id: DashboardItemId::Panel(index),
        rect,
        disclosure_width: None,
        kind: DashboardRectKind::Panel { index },
    }
}

pub(super) fn clip_projected_rect(
    item: ProjectedDashboardRect,
    area: Rect,
    scroll_offset: u64,
) -> Option<DashboardRect> {
    let viewport_bottom = scroll_offset.saturating_add(u64::from(area.height));
    let top = item.rect.y.max(scroll_offset);
    let bottom = item
        .rect
        .y
        .saturating_add(item.rect.height)
        .min(viewport_bottom);
    let x = item.rect.x.max(area.x);
    let right = u32::from(item.rect.x)
        .saturating_add(u32::from(item.rect.width))
        .min(u32::from(area.right()));
    if top >= bottom || u32::from(x) >= right || area.is_empty() {
        return None;
    }
    // Differences fit the viewport, so conversion occurs only after clipping.
    let rect = Rect::new(
        x,
        area.y.saturating_add((top - scroll_offset) as u16),
        (right - u32::from(x)) as u16,
        (bottom - top) as u16,
    );
    let disclosure_rect = item
        .disclosure_width
        .filter(|_| item.rect.y >= scroll_offset)
        .map(|width| Rect::new(rect.x, rect.y, width.min(rect.width), 1));
    Some(DashboardRect {
        id: item.id,
        rect,
        disclosure_rect,
        kind: item.kind,
    })
}
