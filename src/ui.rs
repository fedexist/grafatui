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
use humantime::format_duration;
use ratatui::{
    prelude::*,
    widgets::{
        Axis, Block, Borders, Chart, Clear, Dataset, GraphType, List, ListItem, Paragraph, Wrap,
    },
};
use std::collections::HashMap;

/// Renders the entire application UI into the given frame.
///
/// This function handles the layout of the title bar, charts area, and footer.
/// It delegates the rendering of individual panels to `render_panel`.
pub fn draw_ui(frame: &mut Frame, app: &AppState) {
    let size = frame.area();

    // Layout: title bar, charts area, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(size);

    // Title
    let title_text = format!(
        "{} — range={} step={}  panels={}  {}(r to refresh, +/- range, [] pan, 0 live, q quit)",
        app.title,
        format_duration(app.range),
        format_duration(app.step),
        app.panels.len(),
        if app.is_live() { "" } else { "⏸ PAUSED " }
    );
    let title_block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(title_text).alignment(Alignment::Center));
    frame.render_widget(title_block, chunks[0]);

    // Charts area: use Grafana grid if any panel has it, else fallback to 2-column flow
    let area = chunks[1];
    let charts_block = Block::default().borders(Borders::ALL);
    frame.render_widget(charts_block, area);
    let inner_area = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    if app.mode == AppMode::Fullscreen || app.mode == AppMode::FullscreenInspect {
        if let Some(p) = app.panels.get(app.selected_panel) {
            render_panel(frame, inner_area, p, app, true, app.cursor_x);
        }
    } else {
        let has_grid = app.panels.iter().any(|p| p.grid.is_some());
        let panel_rects = calculate_panel_layout(inner_area, app);

        for (rect, panel_idx) in &panel_rects {
            // eprintln!("Rendering panel {} at {:?}", panel_idx, rect);
            if let Some(p) = app.panels.get(*panel_idx) {
                let is_selected = *panel_idx == app.selected_panel;
                render_panel(frame, *rect, p, app, is_selected, app.cursor_x);
            }
        }

        if !has_grid && app.panels.is_empty() {
            // No panels to render
        } else if has_grid {
            // Check if we need to render extras (panels without grid)
            // The calculate_grid_layout should handle extras too?
            // The original code handled extras by stacking them below.
            // Let's make calculate_grid_layout return extras too.
        }
    }

    // Footer / Status bar
    let errors = app.panels.iter().filter(|p| p.last_error.is_some()).count();
    let panel_count_display =
        if app.mode == AppMode::Fullscreen || app.mode == AppMode::FullscreenInspect {
            "1 (Fullscreen)".to_string()
        } else {
            format!("{}", app.panels.len())
        };

    let mode_display = match app.mode {
        AppMode::Normal => "NORMAL",
        AppMode::Search => "SEARCH",
        AppMode::Fullscreen => "FULLSCREEN",
        AppMode::Inspect => "INSPECT",
        AppMode::FullscreenInspect => "FULLSCREEN INSPECT",
    };

    let summary = format!(
        "Mode: {}{} | Prom: {} | range={} step={:?} refresh={} | panels={} (skipped {}) errors={} | keys: ↑/↓ scroll, r refresh, e export, Ctrl+E record, +/- range, q quit, ? debug:{}",
        mode_display,
        if app.recording.is_some() { " REC" } else { "" },
        app.prometheus.base,
        format_duration(app.range),
        app.step,
        format_duration(app.refresh_every),
        panel_count_display,
        app.skipped_panels,
        errors,
        if app.debug_bar { "on" } else { "off" }
    );

    let mut detail = String::new();
    if let Some(status) = &app.export_status {
        detail = status.clone();
    }
    if app.debug_bar {
        // Choose a debug panel: if we have grid, pick the top-left grid panel; otherwise pick the first panel
        let debug_panel: Option<&PanelState> = if app.panels.iter().any(|p| p.grid.is_some()) {
            app.panels
                .iter()
                .filter(|p| p.grid.is_some())
                .min_by_key(|p| {
                    let g = p.grid.unwrap();
                    (g.y, g.x)
                })
        } else {
            app.panels.first()
        };

        if let Some(p) = debug_panel {
            let url = p.last_url.as_deref().unwrap_or("-");
            let debug_detail = format!(
                "last panel: {} | samples={} | url={} ",
                p.title, p.last_samples, url
            );
            detail = if detail.is_empty() {
                debug_detail
            } else {
                format!("{} | {}", detail, debug_detail)
            };
        }
    }

    if app.mode == AppMode::Inspect {
        if let Some(cx) = app.cursor_x {
            let cursor_time = chrono::DateTime::from_timestamp(cx as i64, 0)
                .map(|dt| dt.format("%H:%M:%S").to_string())
                .unwrap_or_default();
            detail = format!("Cursor: {} | {}", cursor_time, detail);
        }
    }

    let footer = Paragraph::new(format!("{}\n{}", summary, detail)).wrap(Wrap { trim: true });
    frame.render_widget(footer, chunks[2]);

    // Search Popup
    if app.mode == AppMode::Search {
        let area = centered_rect(60, 20, size);
        let block = Block::default()
            .title(" Search Panels ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border_selected));
        frame.render_widget(Clear, area); // Clear background
        frame.render_widget(block, area);

        let inner_area = area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        });
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner_area);

        // Input
        let input = Paragraph::new(format!("> {}", app.search_query))
            .style(Style::default().fg(app.theme.text));
        frame.render_widget(input, chunks[0]);

        // Results
        let results: Vec<ListItem> = app
            .search_results
            .iter()
            .map(|&idx| {
                let p = &app.panels[idx];
                ListItem::new(format!("• {}", p.title))
            })
            .collect();
        let list = List::new(results)
            .block(Block::default().borders(Borders::TOP))
            .highlight_style(
                Style::default()
                    .fg(app.theme.title)
                    .add_modifier(Modifier::BOLD)
                    .bg(app.theme.background), // Optional: add background to make it pop more?
            )
            .highlight_symbol(">> ");

        let mut list_state = ratatui::widgets::ListState::default();
        if !app.search_results.is_empty() {
            list_state.select(Some(0));
        }
        frame.render_stateful_widget(list, chunks[1], &mut list_state);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
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

    let inner_area = chunks[1].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    if app.mode == AppMode::Fullscreen || app.mode == AppMode::FullscreenInspect {
        return vec![(inner_area, app.selected_panel)];
    }

    calculate_panel_layout(inner_area, app)
}

fn calculate_panel_layout(area: Rect, app: &AppState) -> Vec<(Rect, usize)> {
    if app.panels.iter().any(|p| p.grid.is_some()) {
        calculate_grid_layout(area, app)
    } else {
        calculate_two_column_layout(area, app)
    }
}

/// Returns a list of (Rect, panel_index) for all panels to be rendered.
fn calculate_grid_layout(area: Rect, app: &AppState) -> Vec<(Rect, usize)> {
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

fn calculate_two_column_layout(area: Rect, app: &AppState) -> Vec<(Rect, usize)> {
    let indices: Vec<usize> = (0..app.panels.len()).collect();
    calculate_two_column_layout_subset(area, app, &indices)
}

fn calculate_two_column_layout_subset(
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
pub fn hit_test(app: &AppState, area: Rect, x: u16, y: u16) -> Option<(usize, Rect)> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);
    let inner_area = chunks[1].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let pos = ratatui::layout::Position { x, y };

    if !inner_area.contains(pos) {
        return None;
    }

    for (rect, idx) in visible_panel_rects(area, app) {
        if rect.contains(pos) {
            return Some((idx, rect));
        }
    }
    None
}

/// Renders a single panel.
///
/// This function handles:
/// - Drawing the panel border and title.
/// - Rendering the chart with data series.
/// - Drawing the legend (if space permits).
/// - Handling inspection mode (cursor line and values).
/// - Displaying error messages if the panel has an error.
fn render_panel(
    frame: &mut Frame,
    area: Rect,
    p: &PanelState,
    app: &AppState,
    is_selected: bool,
    cursor_x: Option<f64>,
) {
    let theme = &app.theme;
    let border_style = if is_selected {
        Style::default().fg(theme.border_selected)
    } else {
        Style::default().fg(theme.border)
    };

    if let Some(err) = &p.last_error {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(
                format!("{} — ERROR", p.title),
                Style::default().fg(theme.title),
            ));
        let para = Paragraph::new(err.clone())
            .block(block)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(theme.text));
        frame.render_widget(para, area);
        return;
    }

    // Render the outer block (Panel container)
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            p.title.clone(),
            Style::default().fg(theme.title),
        ));
    frame.render_widget(block.clone(), area);

    let inner_area = block.inner(area);

    match p.panel_type {
        crate::app::PanelType::Graph | crate::app::PanelType::Unknown => {
            render_graph_panel(frame, inner_area, p, app, cursor_x);
        }
        crate::app::PanelType::Gauge => {
            render_gauge(frame, inner_area, p, app);
        }
        crate::app::PanelType::BarGauge => {
            render_bar_gauge(frame, inner_area, p, app);
        }
        crate::app::PanelType::Table => {
            render_table(frame, inner_area, p, app);
        }
        crate::app::PanelType::Stat => {
            render_stat(frame, inner_area, p, app);
        }
        crate::app::PanelType::Heatmap => {
            render_heatmap(frame, inner_area, p, app);
        }
    }
}

fn render_graph_panel(
    frame: &mut Frame,
    area: Rect,
    p: &PanelState,
    app: &AppState,
    cursor_x: Option<f64>,
) {
    let theme = &app.theme;
    let use_hash_colors = p.series.len() > theme.palette.len();

    // If inspecting, find values at cursor
    let cursor_values: HashMap<String, f64> = if let Some(cx) = cursor_x {
        p.series
            .iter()
            .filter_map(|s| {
                // Find point closest to cursor_x
                let closest = s.points.iter().min_by(|a, b| {
                    let da = (a.0 - cx).abs();
                    let db = (b.0 - cx).abs();
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                });

                if let Some((ts, val)) = closest {
                    // Only consider if within reasonable distance (e.g. 2 steps)
                    if (ts - cx).abs() <= app.step.as_secs_f64() * 2.0 {
                        Some((s.name.clone(), *val))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    } else {
        HashMap::new()
    };

    // Split inner area into chart and legend
    // If we have series, reserve space for legend
    let legend_height = if !p.series.is_empty() && area.height > 5 {
        2
    } else {
        0
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(legend_height)])
        .split(area);

    let chart_area = chunks[0];
    let legend_area = chunks[1];

    // Determine x bounds from range window (unix seconds)
    // Use app.time_offset to shift the window
    let now = (chrono::Utc::now().timestamp() - app.time_offset.as_secs() as i64) as f64;
    let start = now - app.range.as_secs_f64();

    // Calculate y_bounds once
    let y_bounds = calculate_y_bounds(p);

    // Prepare datasets (without names for the chart itself to avoid built-in legend)
    let mut chart_datasets = Vec::new();
    let mut legend_items = Vec::new();

    // Declare helper datasets to extend their lifetimes
    let mut cursor_dataset = vec![];
    let mut threshold_datasets = vec![];
    let mut threshold_overlay_datasets = Vec::new();

    let mut threshold_labels_info = Vec::new();

    // Generate threshold limit lines
    if let Some(th) = &p.thresholds {
        for step in th.steps.iter().filter(|s| s.value.is_some()) {
            let val = step.value.unwrap();
            let abs_val = match th.mode {
                crate::app::ThresholdMode::Absolute => val,
                crate::app::ThresholdMode::Percentage => {
                    let min = p.min.unwrap_or(0.0);
                    let max = p.max.unwrap_or(100.0);
                    min + (val / 100.0) * (max - min)
                }
            };

            let mut dataset = Vec::new();
            if app.threshold_marker.starts_with("dashed") || th.style.as_deref() == Some("dashed") {
                let points_count = 15; // 15 evenly spaced ticks across any width length
                let step_x = (now - start) / points_count as f64;
                for i in 0..=points_count {
                    let x = start + (i as f64 * step_x);
                    dataset.push((x, abs_val));
                }
            } else {
                dataset.push((start, abs_val));
                dataset.push((now, abs_val));
            }

            threshold_datasets.push(dataset);
            threshold_labels_info.push((abs_val, step.color));
        }

        for (i, step) in th.steps.iter().filter(|s| s.value.is_some()).enumerate() {
            if app.threshold_marker.ends_with("line") {
                // Skips dataset rendering; handled via post-render buffer overwrite
                continue;
            }

            let (marker, graph_type) = match app.threshold_marker.to_lowercase().as_str() {
                "braille" => (ratatui::symbols::Marker::Braille, GraphType::Line),
                "block" => (ratatui::symbols::Marker::Block, GraphType::Line),
                "bar" => (ratatui::symbols::Marker::Bar, GraphType::Line),
                "half-block" => (ratatui::symbols::Marker::HalfBlock, GraphType::Line),
                "quadrant" => (ratatui::symbols::Marker::Quadrant, GraphType::Line),
                "sextant" => (ratatui::symbols::Marker::Sextant, GraphType::Line),
                "octant" => (ratatui::symbols::Marker::Octant, GraphType::Line),
                "dashed" | "dashed-braille" => {
                    (ratatui::symbols::Marker::Braille, GraphType::Scatter)
                }
                "dashed-block" => (ratatui::symbols::Marker::Block, GraphType::Scatter),
                "dashed-bar" => (ratatui::symbols::Marker::Bar, GraphType::Scatter),
                "dashed-half-block" => (ratatui::symbols::Marker::HalfBlock, GraphType::Scatter),
                "dashed-quadrant" => (ratatui::symbols::Marker::Quadrant, GraphType::Scatter),
                "dashed-sextant" => (ratatui::symbols::Marker::Sextant, GraphType::Scatter),
                "dashed-octant" => (ratatui::symbols::Marker::Octant, GraphType::Scatter),
                "dashed-dot" => (ratatui::symbols::Marker::Dot, GraphType::Scatter),
                _ => (ratatui::symbols::Marker::Dot, GraphType::Line),
            };

            threshold_overlay_datasets.push(
                Dataset::default()
                    .name("")
                    .marker(marker)
                    .graph_type(graph_type)
                    .style(Style::default().fg(step.color))
                    .data(&threshold_datasets[i]),
            );
        }
    }

    for (i, s) in p.series.iter().enumerate() {
        let color = if use_hash_colors {
            get_hash_color(&s.name)
        } else {
            theme.palette[i % theme.palette.len()]
        };

        let data = if s.visible { s.points.as_slice() } else { &[] };

        // For legend display
        let mut name = s.name.clone();
        if let Some(val) = cursor_values.get(&s.name) {
            name.push_str(&format!(" ({})", format_si(*val)));
        } else if let Some(val) = s.value {
            name.push_str(&format!(" ({})", format_si(val)));
        }
        if name.is_empty() {
            name = format!("Series {}", i);
        }

        legend_items.push(Span::styled("■ ".to_string(), Style::default().fg(color)));
        legend_items.push(Span::styled(
            format!("{}  ", name),
            Style::default().fg(theme.text),
        ));

        // For chart (no name to avoid legend)
        chart_datasets.push(
            Dataset::default()
                .name("")
                .marker(ratatui::symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(color))
                .data(data),
        );
    }

    // Add cursor line if inspecting
    if let Some(cx) = cursor_x {
        cursor_dataset.push((cx, y_bounds[0]));
        cursor_dataset.push((cx, y_bounds[1]));

        chart_datasets.push(
            Dataset::default()
                .name("")
                .marker(ratatui::symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::White))
                .data(&cursor_dataset),
        );
    }

    let x_labels = vec![
        Span::styled(format_time(start), Style::default().fg(theme.text)),
        Span::styled(format_time(now), Style::default().fg(theme.text)),
    ];

    let y_axis_height = chart_area.height.saturating_sub(1).max(2) as usize;
    let mut y_labels = vec![Span::raw(""); y_axis_height];

    y_labels[0] = Span::styled(format_si(y_bounds[0]), Style::default().fg(theme.text));
    y_labels[y_axis_height - 1] =
        Span::styled(format_si(y_bounds[1]), Style::default().fg(theme.text));

    if y_bounds[1] > y_bounds[0] {
        for (th_val, color) in &threshold_labels_info {
            if *th_val > y_bounds[0] && *th_val < y_bounds[1] {
                let ratio = (*th_val - y_bounds[0]) / (y_bounds[1] - y_bounds[0]);
                let index = (ratio * (y_axis_height - 1) as f64).round() as usize;
                let index = index.min(y_axis_height - 2).max(1);
                y_labels[index] = Span::styled(format_si(*th_val), Style::default().fg(*color));
            }
        }
    }

    // Evaluate y_max_width before moving y_labels into Chart block
    let y_max_width = y_labels.iter().map(|s| s.width() as u16).max().unwrap_or(0);

    let chart = Chart::new(chart_datasets)
        // No block, as we rendered it outside
        .x_axis(
            Axis::default()
                .bounds([start, now])
                .labels(x_labels.clone())
                .style(Style::default().fg(theme.text)),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(Color::Gray))
                .bounds(y_bounds)
                .labels(y_labels.clone()),
        );
    // No legend position needed as we disabled names

    frame.render_widget(chart, chart_area);

    let chart_left = chart_area.left() + y_max_width + 1; // +1 for the | axis line
    let chart_right = chart_area.right();
    let chart_bottom = chart_area.bottom().saturating_sub(2); // x-axis occupies last rows
    let chart_top = chart_area.top();

    // Render threshold markers after chart rendering by merging only onto blank cells.
    // This guarantees data curves keep precedence wherever both map to the same terminal cell.
    if !threshold_overlay_datasets.is_empty() && chart_top <= chart_bottom {
        let threshold_chart = Chart::new(threshold_overlay_datasets)
            .x_axis(
                Axis::default()
                    .bounds([start, now])
                    .labels(x_labels)
                    .style(Style::default().fg(theme.text)),
            )
            .y_axis(
                Axis::default()
                    .style(Style::default().fg(Color::Gray))
                    .bounds(y_bounds)
                    .labels(y_labels),
            );

        let mut threshold_buf = ratatui::buffer::Buffer::empty(chart_area);
        threshold_chart.render(chart_area, &mut threshold_buf);

        let buf = frame.buffer_mut();
        for y in chart_top..=chart_bottom {
            for x in chart_left..chart_right {
                let Some(src_cell) = threshold_buf.cell((x, y)) else {
                    continue;
                };
                if let Some(dst_cell) = buf.cell_mut((x, y)) {
                    overlay_threshold_cell(dst_cell, src_cell);
                }
            }
        }
    }

    // Render custom raw lines by hijacking buffer space
    if app.threshold_marker.ends_with("line") && y_bounds[1] > y_bounds[0] {
        let buf = frame.buffer_mut();

        let chart_h = chart_bottom.saturating_sub(chart_top) as f64;

        if chart_h > 0.0 {
            let is_dashed = app.threshold_marker.starts_with("dashed");
            let line_char = if is_dashed { '-' } else { '─' };

            for (th_val, color) in &threshold_labels_info {
                if *th_val > y_bounds[0] && *th_val < y_bounds[1] {
                    let ratio = (*th_val - y_bounds[0]) / (y_bounds[1] - y_bounds[0]);
                    let y_offset = (ratio * chart_h).round() as u16;
                    let phys_y = chart_bottom.saturating_sub(y_offset);

                    if phys_y >= chart_top && phys_y <= chart_bottom {
                        for x in chart_left..chart_right {
                            if is_dashed && x % 2 == 0 {
                                continue;
                            }
                            if let Some(cell) = buf.cell_mut((x, phys_y)) {
                                if should_draw_threshold_on_cell(cell) {
                                    cell.set_char(line_char)
                                        .set_style(Style::default().fg(*color));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Render custom legend
    if legend_height > 0 {
        let legend = Paragraph::new(Line::from(legend_items)).wrap(Wrap { trim: true });
        frame.render_widget(legend, legend_area);
    }
}

fn should_draw_threshold_on_cell(cell: &ratatui::buffer::Cell) -> bool {
    cell.symbol().chars().all(char::is_whitespace)
}

fn overlay_threshold_cell(dst: &mut ratatui::buffer::Cell, src: &ratatui::buffer::Cell) {
    if should_draw_threshold_on_cell(dst) && !should_draw_threshold_on_cell(src) {
        dst.set_symbol(src.symbol()).set_style(src.style());
    }
}

fn render_gauge(frame: &mut Frame, area: Rect, p: &PanelState, app: &AppState) {
    let theme = &app.theme;

    // Find the latest value from the first visible series
    let (value, name) = p
        .series
        .iter()
        .filter(|s| s.visible)
        .find_map(|s| s.value.map(|v| (v, s.name.clone())))
        .unwrap_or((0.0, "No data".to_string()));

    let min = p.min.unwrap_or(0.0);
    let max = p
        .max
        .unwrap_or(if value > 100.0 { value * 1.2 } else { 100.0 });

    let color = p.get_color_for_value(value).unwrap_or(theme.palette[0]);

    let ratio = if max > min {
        ((value - min) / (max - min)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let gauge = ratatui::widgets::Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
        .ratio(ratio)
        .label(format!("{} ({})", format_si(value), name));

    frame.render_widget(gauge, area);
}

fn render_bar_gauge(frame: &mut Frame, area: Rect, p: &PanelState, app: &AppState) {
    let theme = &app.theme;

    let mut max_label_len = 3;

    let scale = 1000.0;

    // Map intermediate valid series
    let mut valid_series: Vec<_> = p
        .series
        .iter()
        .filter(|s| s.visible && s.value.is_some())
        .collect();

    // Sort descending safely
    valid_series.sort_by(|a, b| {
        let v_a = a.value.unwrap_or(0.0);
        let v_b = b.value.unwrap_or(0.0);
        v_b.partial_cmp(&v_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Truncate based on area width
    let max_bars = (area.width / 4).saturating_sub(1).max(1) as usize;
    valid_series.truncate(max_bars);

    let mut bars = Vec::with_capacity(valid_series.len());

    for s in valid_series {
        let v = s.value.unwrap();
        max_label_len = max_label_len.max(s.name.len());
        let color = p.get_color_for_value(v).unwrap_or(theme.palette[0]);
        let bar = ratatui::widgets::Bar::default()
            .value((v * scale) as u64)
            .text_value(format_si(v))
            .label(ratatui::text::Line::from(s.name.as_str()))
            .style(Style::default().fg(color))
            .value_style(Style::default().fg(theme.text).bg(color));
        bars.push(bar);
    }

    if bars.is_empty() {
        let para = Paragraph::new("No data").style(Style::default().fg(theme.text));
        frame.render_widget(para, area);
        return;
    }

    let bar_width = (area.width / bars.len() as u16)
        .saturating_sub(1)
        .min(max_label_len as u16)
        .max(3);

    let bar_group = ratatui::widgets::BarGroup::default().bars(&bars);

    let bar_chart = ratatui::widgets::BarChart::default()
        .block(Block::default().borders(Borders::NONE))
        .data(bar_group)
        .bar_width(bar_width)
        .bar_gap(1);

    frame.render_widget(bar_chart, area);
}

fn render_table(frame: &mut Frame, area: Rect, p: &PanelState, app: &AppState) {
    let theme = &app.theme;

    let header = ["Series", "Value"];
    let rows: Vec<ratatui::widgets::Row> = p
        .series
        .iter()
        .filter(|s| s.visible)
        .map(|s| {
            let val_str = s.value.map(format_si).unwrap_or_else(|| "-".to_string());
            let color = s
                .value
                .and_then(|v| p.get_color_for_value(v))
                .unwrap_or(theme.text);

            ratatui::widgets::Row::new(vec![
                ratatui::text::Span::styled(s.name.clone(), Style::default().fg(theme.text)),
                ratatui::text::Span::styled(val_str, Style::default().fg(color)),
            ])
        })
        .collect();

    if rows.is_empty() {
        let para = Paragraph::new("No data").style(Style::default().fg(theme.text));
        frame.render_widget(para, area);
        return;
    }

    let table = ratatui::widgets::Table::new(
        rows,
        [Constraint::Percentage(70), Constraint::Percentage(30)],
    )
    .header(
        ratatui::widgets::Row::new(header)
            .style(
                Style::default()
                    .fg(theme.title)
                    .add_modifier(Modifier::BOLD),
            )
            .bottom_margin(1),
    )
    .block(Block::default().borders(Borders::NONE))
    .column_spacing(1);

    frame.render_widget(table, area);
}

fn render_stat(frame: &mut Frame, area: Rect, p: &PanelState, app: &AppState) {
    let theme = &app.theme;

    // Find the latest value from the first visible series
    let (value, name) = p
        .series
        .iter()
        .filter(|s| s.visible)
        .find_map(|s| s.value.map(|v| (v, s.name.clone())))
        .unwrap_or((0.0, "No data".to_string()));

    let color = p.get_color_for_value(value).unwrap_or(theme.palette[0]);

    // Split area into value (top) and sparkline (bottom)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Render Big Value
    let val_str = format_si(value);
    let big_value = Paragraph::new(val_str)
        .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::NONE));

    frame.render_widget(big_value, chunks[0]);

    // Render Sparkline
    if let Some(s) = p.series.iter().find(|s| s.visible && s.name == name) {
        let data: Vec<u64> = s.points.iter().map(|(_, v)| *v as u64).collect();
        let sparkline = ratatui::widgets::Sparkline::default()
            .block(Block::default().borders(Borders::NONE))
            .data(&data)
            .style(Style::default().fg(color));
        frame.render_widget(sparkline, chunks[1]);
    }
}

fn render_heatmap(frame: &mut Frame, area: Rect, p: &PanelState, app: &AppState) {
    let theme = &app.theme;

    if p.series.is_empty() {
        let para = Paragraph::new("No data").style(Style::default().fg(theme.text));
        frame.render_widget(para, area);
        return;
    }

    // For heatmap, we'll display each series as a row, with time buckets as columns
    // Each cell is colored based on value intensity

    let rows_available = area.height.saturating_sub(2) as usize; // Reserve space for labels
    let cols_available = area.width as usize;

    if rows_available == 0 || cols_available == 0 {
        return;
    }

    // Limit to visible series
    let visible_series: Vec<_> = p.series.iter().filter(|s| s.visible).collect();
    if visible_series.is_empty() {
        let para = Paragraph::new("No visible series").style(Style::default().fg(theme.text));
        frame.render_widget(para, area);
        return;
    }

    // Calculate global min/max for color scaling
    let (mut global_min, mut global_max) = (f64::MAX, f64::MIN);
    for s in &visible_series {
        for (_, v) in &s.points {
            if v.is_finite() {
                global_min = global_min.min(*v);
                global_max = global_max.max(*v);
            }
        }
    }

    if !global_min.is_finite() || !global_max.is_finite() || global_min == global_max {
        let para = Paragraph::new("Invalid data range").style(Style::default().fg(theme.text));
        frame.render_widget(para, area);
        return;
    }

    // Prepare text lines for the heatmap
    let mut lines = Vec::new();

    for series in visible_series.iter().take(rows_available) {
        let mut spans = Vec::new();

        // Downsample points to fit available columns
        let total_points = series.points.len();
        if total_points == 0 {
            continue;
        }

        let step = (total_points as f64 / cols_available as f64).max(1.0);

        for col_idx in 0..cols_available {
            let point_idx = ((col_idx as f64 * step) as usize).min(total_points - 1);
            let (_, value) = series.points[point_idx];

            // Map value to color intensity (from blue/cold to red/hot)
            let color = if value.is_finite() {
                let normalized = ((value - global_min) / (global_max - global_min)).clamp(0.0, 1.0);
                value_to_heatmap_color(normalized)
            } else {
                Color::DarkGray
            };

            spans.push(Span::styled("█", Style::default().fg(color)));
        }

        lines.push(Line::from(spans));
    }

    if lines.is_empty() {
        let para = Paragraph::new("No data to display").style(Style::default().fg(theme.text));
        frame.render_widget(para, area);
        return;
    }

    let heatmap_widget = Paragraph::new(lines).block(Block::default().borders(Borders::NONE));

    frame.render_widget(heatmap_widget, area);
}

/// Maps a normalized value (0.0-1.0) to a heatmap color (blue -> green -> yellow -> red)
fn value_to_heatmap_color(normalized: f64) -> Color {
    // Use a simple color gradient for heatmap
    // 0.0 = Blue (cold), 0.5 = Yellow, 1.0 = Red (hot)
    if normalized < 0.33 {
        // Blue to Cyan
        Color::Cyan
    } else if normalized < 0.66 {
        // Yellow/Green
        Color::Yellow
    } else {
        // Red/Magenta for hot values
        Color::Red
    }
}

pub(crate) fn calculate_y_bounds(p: &PanelState) -> [f64; 2] {
    let mut min = f64::MAX;
    let mut max = f64::MIN;
    let mut has_data = false;

    for s in &p.series {
        if !s.visible {
            continue;
        }
        for &(_, v) in &s.points {
            if !v.is_finite() {
                continue;
            }
            if v < min {
                min = v;
            }
            if v > max {
                max = v;
            }
            has_data = true;
        }
    }

    if !has_data {
        return [0.0, 1.0];
    }

    if min == max {
        min -= 1.0;
        max += 1.0;
    }

    if p.y_axis_mode == crate::app::YAxisMode::ZeroBased {
        if min > 0.0 {
            min = 0.0;
        } else if max < 0.0 {
            max = 0.0;
        }
    }

    // Add some padding
    let range = max - min;
    [min - range * 0.05, max + range * 0.05]
}

pub(crate) fn format_si(val: f64) -> String {
    let abs = val.abs();
    if abs >= 1e9 {
        format!("{:.2}G", val / 1e9)
    } else if abs >= 1e6 {
        format!("{:.2}M", val / 1e6)
    } else if abs >= 1e3 {
        format!("{:.2}k", val / 1e3)
    } else {
        format!("{:.2}", val)
    }
}

pub(crate) fn format_time(ts: f64) -> String {
    use chrono::TimeZone;
    if let Some(dt) = chrono::Utc.timestamp_opt(ts as i64, 0).single() {
        dt.format("%H:%M:%S").to_string()
    } else {
        format!("{}", ts)
    }
}

/// Generate a color from a string using hash-based approach.
/// Uses HSL color space to ensure visually distinct, vibrant colors.
pub(crate) fn get_hash_color(name: &str) -> Color {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    let hash = hasher.finish();

    // Use HSL color space for better color distribution
    // Hue: use the hash to get different hues (0-360 degrees)
    let hue = (hash % 360) as f32;

    // Saturation: keep high for vibrant colors (60-90%)
    let saturation = 60.0 + ((hash >> 8) % 30) as f32;

    // Lightness: keep in a range that's visible on both light and dark backgrounds (45-65%)
    let lightness = 45.0 + ((hash >> 16) % 20) as f32;

    hsl_to_rgb(hue, saturation, lightness)
}

/// Convert HSL to RGB color for ratatui.
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Color {
    let s = s / 100.0;
    let l = l / 100.0;

    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    Color::Rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{SeriesView, YAxisMode};

    fn create_test_panel() -> PanelState {
        PanelState {
            title: "test".to_string(),
            exprs: vec![],
            legends: vec![],
            series: vec![],
            last_error: None,
            last_url: None,
            last_samples: 0,
            grid: None,
            y_axis_mode: YAxisMode::Auto,
            panel_type: crate::app::PanelType::Graph,
            thresholds: None,
            min: None,
            max: None,
        }
    }

    #[test]
    fn test_calculate_y_bounds_basic() {
        let mut p = create_test_panel();
        p.series.push(SeriesView {
            name: "test".to_string(),
            value: None,
            points: vec![(0.0, 10.0), (1.0, 20.0)],
            visible: true,
        });

        let bounds = calculate_y_bounds(&p);
        assert!(bounds[0] < 10.0);
        assert!(bounds[1] > 20.0);
    }

    #[test]
    fn test_calculate_y_bounds_nan() {
        let mut p = create_test_panel();
        p.series.push(SeriesView {
            name: "test".to_string(),
            value: None,
            points: vec![(0.0, 10.0), (1.0, f64::NAN), (2.0, 20.0)],
            visible: true,
        });

        let bounds = calculate_y_bounds(&p);
        assert!(bounds[0] < 10.0); // Should ignore NAN
        assert!(bounds[1] > 20.0);
    }

    #[test]
    fn test_calculate_y_bounds_infinity() {
        let mut p = create_test_panel();
        p.series.push(SeriesView {
            name: "test".to_string(),
            value: None,
            points: vec![(0.0, 10.0), (1.0, f64::INFINITY), (2.0, 20.0)],
            visible: true,
        });

        let bounds = calculate_y_bounds(&p);
        assert!(bounds[0] < 10.0); // Should ignore INFINITY
        assert!(bounds[1] > 20.0);
    }

    #[test]
    fn test_calculate_y_bounds_zero_based() {
        let mut p = create_test_panel();
        p.y_axis_mode = YAxisMode::ZeroBased;
        p.series.push(SeriesView {
            name: "test".to_string(),
            value: None,
            points: vec![(0.0, 10.0), (1.0, 20.0)],
            visible: true,
        });

        let bounds = calculate_y_bounds(&p);
        // Range is 0.0 to 20.0. Padding is 5% of 20.0 = 1.0.
        // So min should be 0.0 - 1.0 = -1.0.
        assert_eq!(bounds[0], -1.0);
        assert!(bounds[1] > 20.0);
    }

    #[test]
    fn test_should_draw_threshold_on_cell_empty() {
        let cell = ratatui::buffer::Cell::default();
        assert!(should_draw_threshold_on_cell(&cell));
    }

    #[test]
    fn test_should_draw_threshold_on_cell_filled() {
        let mut cell = ratatui::buffer::Cell::default();
        cell.set_char('x');
        assert!(!should_draw_threshold_on_cell(&cell));
    }

    #[test]
    fn test_overlay_threshold_cell_copies_when_destination_is_empty() {
        let mut dst = ratatui::buffer::Cell::default();
        let mut src = ratatui::buffer::Cell::default();
        src.set_char('-').set_style(Style::default().fg(Color::Red));

        overlay_threshold_cell(&mut dst, &src);

        assert_eq!(dst.symbol(), "-");
        assert_eq!(dst.style().fg, Some(Color::Red));
    }

    #[test]
    fn test_overlay_threshold_cell_keeps_existing_destination_marker() {
        let mut dst = ratatui::buffer::Cell::default();
        dst.set_char('x')
            .set_style(Style::default().fg(Color::LightBlue));
        let mut src = ratatui::buffer::Cell::default();
        src.set_char('-').set_style(Style::default().fg(Color::Red));

        overlay_threshold_cell(&mut dst, &src);

        assert_eq!(dst.symbol(), "x");
        assert_eq!(dst.style().fg, Some(Color::LightBlue));
    }
}
