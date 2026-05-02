/*
 * Copyright 2025 Federico D'Ambrosio
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

use crate::app::{AppMode, AppState, PanelState};
use ratatui::prelude::*;

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

pub(crate) fn visible_panel_rects(area: Rect, app: &AppState) -> Vec<(Rect, usize)> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);

    let charts_area = chunks[1];
    let inner_area = charts_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    if app.mode == AppMode::Fullscreen || app.mode == AppMode::FullscreenInspect {
        return vec![(inner_area, app.selected_panel)];
    }

    if app.panels.iter().any(|p| p.grid.is_some()) {
        calculate_grid_layout(inner_area, app)
    } else {
        calculate_two_column_layout(inner_area, app)
    }
}

/// Determines which panel is located at the given coordinates.
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
/// An `Option` containing a tuple of `(panel_index, panel_rect)` if a panel was hit.
pub(crate) fn hit_test(app: &AppState, area: Rect, x: u16, y: u16) -> Option<(usize, Rect)> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);

    let charts_area = chunks[1];
    let inner_area = charts_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    if !inner_area.contains(ratatui::layout::Position { x, y }) {
        return None;
    }

    for (rect, idx) in visible_panel_rects(area, app) {
        if rect.contains(ratatui::layout::Position { x, y }) {
            return Some((idx, rect));
        }
    }
    None
}
