/*
 * Copyright 2026 Federico D'Ambrosio
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use crate::{
    app::{AppMode, AppState, PanelState},
    dashboard::{DashboardItemId, DashboardLayoutItem, RowId},
};
use ratatui::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DashboardRect {
    pub(crate) id: DashboardItemId,
    pub(crate) rect: Rect,
    pub(crate) disclosure_rect: Option<Rect>,
    pub(crate) kind: DashboardRectKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DashboardRectKind {
    Row {
        row_id: RowId,
        depth: usize,
        collapsed: bool,
    },
    Panel {
        index: usize,
    },
}

pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

/// Returns a list of (Rect, panel_index) for all panels to be rendered.
pub(crate) fn calculate_grid_layout(area: Rect, app: &AppState) -> Vec<(Rect, usize)> {
    let mut results = Vec::new();

    // Grafana uses a 24-column grid; y/h units are arbitrary grid rows.
    let grid_cols: u16 = 24;
    let cell_w = std::cmp::max(1, area.width / grid_cols);
    // Heuristic: choose a usable cell height from terminal height (min 3 rows per h-unit)
    let cell_h = std::cmp::max(3, area.height / 24);

    // Render grid-backed panels with scroll offset
    let scroll_offset = app.vertical_scroll as u16 * cell_h;

    for (i, p) in app.panels.iter().enumerate() {
        if let Some(g) = p.grid {
            if g.x < 0 || g.y < 0 || g.w <= 0 || g.h <= 0 {
                continue;
            }
            let x = area.x.saturating_add((g.x as u16).saturating_mul(cell_w));
            let y_absolute = (g.y as u16).saturating_mul(cell_h);

            // Apply scroll offset
            if y_absolute < scroll_offset {
                // Panel is scrolled out of view at the top
                continue;
            }
            let y = area
                .y
                .saturating_add(y_absolute.saturating_sub(scroll_offset));

            let w = (g.w as u16).saturating_mul(cell_w);
            let h = (g.h as u16).saturating_mul(cell_h);

            // Skip panels that are completely below the visible area
            if y >= area.bottom() {
                continue;
            }

            // Clamp to area
            let rect = Rect {
                x,
                y,
                width: w.min(area.right().saturating_sub(x)),
                height: h.min(area.bottom().saturating_sub(y)),
            };
            if rect.width >= 8 && rect.height >= 4 {
                results.push((rect, i));
            }
        }
    }

    // Extras (panels without grid)
    let extras: Vec<(usize, &PanelState)> = app
        .panels
        .iter()
        .enumerate()
        .filter(|(_, p)| p.grid.is_none())
        .collect();
    if !extras.is_empty() {
        // Place extras in a vertical stack under the grid.
        let max_y_h = app
            .panels
            .iter()
            .filter_map(|p| {
                let g = p.grid?;
                Some(g.y + g.h)
            })
            .max()
            .unwrap_or(0);

        let start_y_px = area
            .y
            .saturating_add((max_y_h as u16).saturating_mul(cell_h));

        if start_y_px < area.bottom() {
            let extras_area = Rect {
                x: area.x,
                y: start_y_px,
                width: area.width,
                height: area.bottom().saturating_sub(start_y_px),
            };

            // Reuse two-column layout for extras
            // We need to pass the subset of panels but keep their original indices.
            let extra_indices: Vec<usize> = extras.iter().map(|(i, _)| *i).collect();
            let extra_rects = calculate_two_column_layout_subset(extras_area, app, &extra_indices);
            results.extend(extra_rects);
        }
    }

    results
}

pub(crate) fn calculate_two_column_layout(area: Rect, app: &AppState) -> Vec<(Rect, usize)> {
    let indices: Vec<usize> = (0..app.panels.len()).collect();
    calculate_two_column_layout_subset(area, app, &indices)
}

pub(crate) fn calculate_two_column_layout_subset(
    area: Rect,
    app: &AppState,
    panel_indices: &[usize],
) -> Vec<(Rect, usize)> {
    let mut results = Vec::new();
    if panel_indices.is_empty() {
        return results;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let panel_height = 12u16;
    let rows_fit = (area.height / panel_height).saturating_mul(2).max(1) as usize;

    // Scroll handling
    // If we are rendering the main list (not extras), we use app.vertical_scroll.
    // If we are rendering extras, we might want independent scroll or just show what fits.
    // For now, use app.vertical_scroll only if we are rendering the full list (heuristic).
    // Or better: always use it, but clamp it.

    let start = app
        .vertical_scroll
        .min(panel_indices.len().saturating_sub(rows_fit));
    let end = (start + rows_fit).min(panel_indices.len());

    let visible_indices = &panel_indices[start..end];

    let mut left_indices = Vec::new();
    let mut right_indices = Vec::new();

    for (i, &original_idx) in visible_indices.iter().enumerate() {
        if i % 2 == 0 {
            left_indices.push(original_idx);
        } else {
            right_indices.push(original_idx);
        }
    }

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(panel_height); left_indices.len()])
        .split(cols[0]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(panel_height); right_indices.len()])
        .split(cols[1]);

    for (rect, &idx) in left_chunks.iter().zip(left_indices.iter()) {
        results.push((*rect, idx));
    }
    for (rect, &idx) in right_chunks.iter().zip(right_indices.iter()) {
        results.push((*rect, idx));
    }

    results
}

fn dashboard_inner_area(area: Rect) -> Rect {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);

    chunks[1].inner(Margin {
        vertical: 1,
        horizontal: 1,
    })
}

pub(crate) fn visible_dashboard_rects(area: Rect, app: &AppState) -> Vec<DashboardRect> {
    let inner_area = dashboard_inner_area(area);

    if app.mode == AppMode::Fullscreen || app.mode == AppMode::FullscreenInspect {
        return app
            .selected_panel_index()
            .map(|index| {
                vec![DashboardRect {
                    id: DashboardItemId::Panel(index),
                    rect: inner_area,
                    disclosure_rect: None,
                    kind: DashboardRectKind::Panel { index },
                }]
            })
            .unwrap_or_default();
    }

    let visible = app.layout.visible_items();
    if !visible
        .iter()
        .any(|item| matches!(item.id, DashboardItemId::Row(_)))
    {
        let structurally_flat = app
            .layout
            .items
            .iter()
            .all(|item| matches!(item, DashboardLayoutItem::Panel(_)));
        if structurally_flat {
            let panel_rects = if app.panels.iter().any(|p| p.grid.is_some()) {
                calculate_grid_layout(inner_area, app)
            } else {
                calculate_two_column_layout(inner_area, app)
            };
            return panel_rects
                .into_iter()
                .map(|(rect, index)| panel_rect(index, rect))
                .collect();
        }

        let cell_h = std::cmp::max(3, inner_area.height / 24);
        let mut projected = Vec::new();
        let mut cursor_y = inner_area.y;
        for group in transparent_panel_groups(&app.layout.items) {
            cursor_y =
                project_panel_group(inner_area, cursor_y, cell_h, app, &group, &mut projected);
        }
        let scroll_offset = u16::try_from(app.vertical_scroll)
            .unwrap_or(u16::MAX)
            .saturating_mul(cell_h);
        return projected
            .into_iter()
            .filter_map(|item| clip_scrolled_rect(item, inner_area, scroll_offset))
            .collect();
    }

    let cell_h = std::cmp::max(3, inner_area.height / 24);
    let mut projected = Vec::new();
    let mut cursor_y = inner_area.y;
    let mut position = 0;
    while position < visible.len() {
        match visible[position].id {
            DashboardItemId::Row(row_id) => {
                let Some(row) = app.layout.row(row_id) else {
                    position += 1;
                    continue;
                };
                let rect = Rect::new(inner_area.x, cursor_y, inner_area.width, 1);
                projected.push(DashboardRect {
                    id: DashboardItemId::Row(row_id),
                    rect,
                    disclosure_rect: Some(Rect::new(rect.x, rect.y, rect.width.min(1), 1)),
                    kind: DashboardRectKind::Row {
                        row_id,
                        depth: visible[position].depth,
                        collapsed: row.collapsed,
                    },
                });
                cursor_y = cursor_y.saturating_add(1);
                position += 1;
            }
            DashboardItemId::Panel(_) => {
                let start = position;
                while position < visible.len()
                    && matches!(visible[position].id, DashboardItemId::Panel(_))
                {
                    position += 1;
                }
                let indices = visible[start..position]
                    .iter()
                    .filter_map(|item| match item.id {
                        DashboardItemId::Panel(index) => Some(index),
                        DashboardItemId::Row(_) => None,
                    })
                    .collect::<Vec<_>>();
                cursor_y = project_panel_group(
                    inner_area,
                    cursor_y,
                    cell_h,
                    app,
                    &indices,
                    &mut projected,
                );
            }
        }
    }

    let scroll_offset = u16::try_from(app.vertical_scroll)
        .unwrap_or(u16::MAX)
        .saturating_mul(cell_h);
    projected
        .into_iter()
        .filter_map(|item| clip_scrolled_rect(item, inner_area, scroll_offset))
        .collect()
}

fn transparent_panel_groups(items: &[DashboardLayoutItem]) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();
    let mut panels = Vec::new();
    for item in items {
        match item {
            DashboardLayoutItem::Panel(index) => panels.push(*index),
            DashboardLayoutItem::Row(row) => {
                if !panels.is_empty() {
                    groups.push(std::mem::take(&mut panels));
                }
                if row.hidden_header || !row.collapsed {
                    groups.extend(transparent_panel_groups(&row.children));
                }
            }
        }
    }
    if !panels.is_empty() {
        groups.push(panels);
    }
    groups
}

fn project_panel_group(
    area: Rect,
    origin_y: u16,
    cell_h: u16,
    app: &AppState,
    panel_indices: &[usize],
    output: &mut Vec<DashboardRect>,
) -> u16 {
    let has_grid = panel_indices.iter().any(|&index| {
        app.panels
            .get(index)
            .is_some_and(|panel| panel.grid.is_some())
    });
    if !has_grid {
        let columns = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(Rect::new(area.x, origin_y, area.width, u16::MAX - origin_y));
        for (position, &index) in panel_indices.iter().enumerate() {
            let row = (position / 2) as u16;
            let column = position % 2;
            let rect = Rect::new(
                columns[column].x,
                origin_y.saturating_add(row.saturating_mul(12)),
                columns[column].width,
                12,
            );
            output.push(panel_rect(index, rect));
        }
        let rows = panel_indices.len().div_ceil(2) as u16;
        return origin_y.saturating_add(rows.saturating_mul(12));
    }

    let cell_w = std::cmp::max(1, area.width / 24);
    let mut grid_height = 0u16;
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
        let x = area
            .x
            .saturating_add((grid.x as u16).saturating_mul(cell_w));
        let y = origin_y.saturating_add((grid.y as u16).saturating_mul(cell_h));
        let rect = Rect::new(
            x,
            y,
            (grid.w as u16)
                .saturating_mul(cell_w)
                .min(area.right().saturating_sub(x)),
            (grid.h as u16).saturating_mul(cell_h),
        );
        if rect.width >= 8 && rect.height >= 4 {
            output.push(panel_rect(index, rect));
        }
        grid_height = grid_height.max(
            (grid.y as u16)
                .saturating_add(grid.h as u16)
                .saturating_mul(cell_h),
        );
    }

    let extras_origin = origin_y.saturating_add(grid_height);
    let columns =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(
            Rect::new(area.x, extras_origin, area.width, u16::MAX - extras_origin),
        );
    for (position, index) in extras.iter().copied().enumerate() {
        let row = (position / 2) as u16;
        let column = position % 2;
        output.push(panel_rect(
            index,
            Rect::new(
                columns[column].x,
                extras_origin.saturating_add(row.saturating_mul(12)),
                columns[column].width,
                12,
            ),
        ));
    }
    origin_y
        .saturating_add(grid_height)
        .saturating_add((extras.len().div_ceil(2) as u16).saturating_mul(12))
}

fn panel_rect(index: usize, rect: Rect) -> DashboardRect {
    DashboardRect {
        id: DashboardItemId::Panel(index),
        rect,
        disclosure_rect: None,
        kind: DashboardRectKind::Panel { index },
    }
}

fn clip_scrolled_rect(
    mut item: DashboardRect,
    area: Rect,
    scroll_offset: u16,
) -> Option<DashboardRect> {
    let top = item.rect.y.saturating_sub(scroll_offset);
    let bottom = item.rect.bottom().saturating_sub(scroll_offset);
    if bottom <= area.y || top >= area.bottom() {
        return None;
    }
    item.rect.y = top.max(area.y);
    item.rect.height = bottom.min(area.bottom()).saturating_sub(item.rect.y);
    item.disclosure_rect = item
        .disclosure_rect
        .and_then(|rect| {
            rect.y
                .checked_sub(scroll_offset)
                .map(|y| Rect { y, ..rect })
        })
        .filter(|rect| {
            area.contains(Position {
                x: rect.x,
                y: rect.y,
            })
        });
    Some(item)
}

#[allow(dead_code)]
pub(crate) fn visible_panel_rects(area: Rect, app: &AppState) -> Vec<(Rect, usize)> {
    visible_dashboard_rects(area, app)
        .into_iter()
        .filter_map(|item| match item.kind {
            DashboardRectKind::Panel { index } => Some((item.rect, index)),
            DashboardRectKind::Row { .. } => None,
        })
        .collect()
}

/// Determines which visible dashboard item is located at the given coordinates.
///
/// # Arguments
///
/// * `app` - The application state.
/// * `area` - The total area available for charts.
/// * `x` - The x-coordinate of the mouse event.
/// * `y` - The y-coordinate of the mouse event.
///
/// # Returns
///
/// A copy of the hit rectangle, including its disclosure target for rows.
pub(crate) fn hit_test(app: &AppState, area: Rect, x: u16, y: u16) -> Option<DashboardRect> {
    let inner_area = dashboard_inner_area(area);

    if !inner_area.contains(ratatui::layout::Position { x, y }) {
        return None;
    }

    visible_dashboard_rects(area, app)
        .into_iter()
        .find(|item| item.rect.contains(ratatui::layout::Position { x, y }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{GridUnit, PanelOptions, PanelType, YAxisMode},
        dashboard::{DashboardItemId, DashboardLayout, DashboardLayoutItem, DashboardRow, RowId},
        export::ExportOptions,
        prom::PromClient,
        theme::Theme,
        ui::DisplayFormat,
    };
    use std::time::Duration;

    fn panel(title: &str, grid: Option<GridUnit>) -> PanelState {
        PanelState {
            title: title.to_string(),
            exprs: vec![],
            legends: vec![],
            query_modes: vec![],
            series: vec![],
            last_error: None,
            last_url: None,
            last_samples: 0,
            grid,
            y_axis_mode: YAxisMode::Auto,
            panel_type: PanelType::Graph,
            thresholds: None,
            min: None,
            max: None,
            autogrid: None,
            display: DisplayFormat::default(),
            options: PanelOptions::None,
        }
    }

    fn app_with(panels: Vec<PanelState>, layout: DashboardLayout) -> AppState {
        let mut app = AppState::new(
            PromClient::new("http://127.0.0.1:9".to_string()),
            Duration::from_secs(300),
            Duration::from_secs(5),
            Duration::from_secs(60),
            "Rows".to_string(),
            panels,
            0,
            Theme::default(),
            "dashed".to_string(),
            ExportOptions::default(),
        );
        app.apply_layout(layout);
        app
    }

    fn nested_row_app() -> AppState {
        app_with(
            vec![
                panel(
                    "Visible child",
                    Some(GridUnit {
                        x: 0,
                        y: 0,
                        w: 24,
                        h: 2,
                    }),
                ),
                panel(
                    "Collapsed child",
                    Some(GridUnit {
                        x: 0,
                        y: 0,
                        w: 24,
                        h: 2,
                    }),
                ),
            ],
            DashboardLayout::new(vec![DashboardLayoutItem::Row(DashboardRow::new(
                RowId::new(0),
                "Expanded",
                false,
                false,
                vec![
                    DashboardLayoutItem::Panel(0),
                    DashboardLayoutItem::Row(DashboardRow::new(
                        RowId::new(1),
                        "Nested",
                        true,
                        false,
                        vec![DashboardLayoutItem::Panel(1)],
                    )),
                ],
            ))]),
        )
    }

    #[test]
    fn row_projection_offsets_nested_grid_panels_and_collapses_subtrees() {
        let app = nested_row_app();

        let rects = visible_dashboard_rects(Rect::new(0, 0, 120, 50), &app);

        assert_eq!(rects.len(), 3);
        assert!(matches!(
            rects[0].kind,
            DashboardRectKind::Row {
                row_id,
                depth: 0,
                collapsed: false,
            } if row_id == RowId::new(0)
        ));
        assert_eq!(rects[0].rect.height, 1);
        assert!(matches!(
            rects[1].kind,
            DashboardRectKind::Panel { index: 0 }
        ));
        assert_eq!(rects[1].id, DashboardItemId::Panel(0));
        assert_eq!(rects[1].rect.y, rects[0].rect.bottom());
        assert!(matches!(
            rects[2].kind,
            DashboardRectKind::Row {
                row_id,
                depth: 1,
                collapsed: true,
            } if row_id == RowId::new(1)
        ));
        assert_eq!(rects[2].rect.y, rects[1].rect.bottom());
        assert!(
            rects
                .iter()
                .all(|item| item.id != DashboardItemId::Panel(1))
        );
    }

    #[test]
    fn hidden_header_row_is_layout_transparent() {
        let grid = GridUnit {
            x: 3,
            y: 2,
            w: 9,
            h: 2,
        };
        let hidden = app_with(
            vec![panel("Panel", Some(grid))],
            DashboardLayout::new(vec![DashboardLayoutItem::Row(DashboardRow::new(
                RowId::new(0),
                "Hidden",
                true,
                true,
                vec![DashboardLayoutItem::Panel(0)],
            ))]),
        );
        let flat = app_with(vec![panel("Panel", Some(grid))], DashboardLayout::flat(1));

        let hidden_rects = visible_dashboard_rects(Rect::new(0, 0, 120, 50), &hidden);
        let flat_rects = visible_dashboard_rects(Rect::new(0, 0, 120, 50), &flat);

        assert_eq!(hidden_rects.len(), 1);
        assert_eq!(hidden_rects[0].id, DashboardItemId::Panel(0));
        assert_eq!(hidden_rects[0].rect, flat_rects[0].rect);
        assert!(hidden_rects[0].disclosure_rect.is_none());
    }

    #[test]
    fn sibling_hidden_header_rows_flow_their_local_grid_children() {
        let grid = GridUnit {
            x: 0,
            y: 0,
            w: 24,
            h: 2,
        };
        let app = app_with(
            vec![
                panel("First child", Some(grid)),
                panel("Second child", Some(grid)),
            ],
            DashboardLayout::new(vec![
                DashboardLayoutItem::Row(DashboardRow::new(
                    RowId::new(0),
                    "Hidden first",
                    false,
                    true,
                    vec![DashboardLayoutItem::Panel(0)],
                )),
                DashboardLayoutItem::Row(DashboardRow::new(
                    RowId::new(1),
                    "Hidden second",
                    false,
                    true,
                    vec![DashboardLayoutItem::Panel(1)],
                )),
            ]),
        );

        let rects = visible_dashboard_rects(Rect::new(0, 0, 120, 50), &app);

        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].id, DashboardItemId::Panel(0));
        assert_eq!(rects[1].id, DashboardItemId::Panel(1));
        assert!(rects[0].rect.bottom() <= rects[1].rect.y);
    }

    #[test]
    fn row_free_projection_preserves_legacy_panel_vector_growth() {
        let mut app = app_with(vec![panel("CPU", None)], DashboardLayout::flat(1));
        app.panels.push(panel("Memory", None));

        let rects = visible_dashboard_rects(Rect::new(0, 0, 120, 50), &app);
        let panel_indices = rects
            .into_iter()
            .filter_map(|item| match item.kind {
                DashboardRectKind::Panel { index } => Some(index),
                DashboardRectKind::Row { .. } => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(panel_indices, vec![0, 1]);
    }

    #[test]
    fn hit_test_returns_row_and_separate_disclosure_target() {
        let app = nested_row_app();
        let area = Rect::new(0, 0, 100, 40);
        let row = visible_dashboard_rects(area, &app)
            .into_iter()
            .find(|item| item.id == DashboardItemId::Row(RowId::new(0)))
            .unwrap();
        let disclosure = row.disclosure_rect.unwrap();

        let disclosure_hit = hit_test(&app, area, disclosure.x, disclosure.y).unwrap();
        let header_hit = hit_test(&app, area, row.rect.right() - 1, row.rect.y).unwrap();

        assert_eq!(disclosure_hit.id, DashboardItemId::Row(RowId::new(0)));
        assert_eq!(disclosure_hit.disclosure_rect, Some(disclosure));
        assert_eq!(header_hit.id, DashboardItemId::Row(RowId::new(0)));
        assert!(!disclosure.contains(Position {
            x: row.rect.right() - 1,
            y: row.rect.y,
        }));
    }
}
