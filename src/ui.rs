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

        let panel_rects = if has_grid {
            calculate_grid_layout(inner_area, app)
        } else {
            calculate_two_column_layout(inner_area, app)
        };

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
        "Mode: {} | Prom: {} | range={} step={:?} refresh={} | grid={} | panels={} (skipped {}) errors={} | keys: ↑/↓ scroll, r refresh, +/- range, q quit, ? debug:{}",
        mode_display,
        app.prometheus.base,
        format_duration(app.range),
        app.step,
        format_duration(app.refresh_every),
        if app.autogrid_enabled { "on" } else { "off" },
        panel_count_display,
        app.skipped_panels,
        errors,
        if app.debug_bar { "on" } else { "off" }
    );

    let mut detail = String::new();
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
            detail = format!(
                "last panel: {} | samples={} | url={} ",
                p.title, p.last_samples, url
            );
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
    // Replicate main layout
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

    if app.mode == AppMode::Fullscreen || app.mode == AppMode::FullscreenInspect {
        return Some((app.selected_panel, inner_area));
    }

    let has_grid = app.panels.iter().any(|p| p.grid.is_some());
    let panel_rects = if has_grid {
        calculate_grid_layout(inner_area, app)
    } else {
        calculate_two_column_layout(inner_area, app)
    };

    for (rect, idx) in panel_rects {
        if rect.contains(ratatui::layout::Position { x, y }) {
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

    // Determine x bounds from the last refreshed query window.
    let (start, now) = app.time_bounds();

    // Calculate y_bounds once
    let y_bounds = calculate_y_bounds(p);
    let show_autogrid = app.autogrid_enabled && p.autogrid.unwrap_or(true);

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

    let time_range_secs = now - start;
    let x_labels = vec![
        Span::styled(
            format_axis_time(start, time_range_secs),
            Style::default().fg(theme.text),
        ),
        Span::styled(
            format_axis_time(now, time_range_secs),
            Style::default().fg(theme.text),
        ),
    ];

    let chart_bottom = chart_area.bottom().saturating_sub(2); // x-axis occupies last rows
    let chart_top = chart_area.top();
    let plot_height = chart_bottom.saturating_sub(chart_top).saturating_add(1);
    let y_axis_height = usize::from(plot_height).max(2);
    let mut y_labels = vec![Span::raw(""); y_axis_height];
    let autogrid_value_ticks = if show_autogrid {
        calculate_value_grid_ticks(y_bounds, plot_height)
    } else {
        Vec::new()
    };

    y_labels[0] = Span::styled(format_si(y_bounds[0]), Style::default().fg(theme.text));
    y_labels[y_axis_height - 1] =
        Span::styled(format_si(y_bounds[1]), Style::default().fg(theme.text));

    // Evaluate y_max_width before moving y_labels into Chart block
    let y_max_width = y_label_width(&y_labels, &autogrid_value_ticks, &threshold_labels_info);

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
    let plot_bounds = PlotBounds {
        left: chart_left,
        right: chart_right,
        top: chart_top,
        bottom: chart_bottom,
    };

    // Render threshold markers after chart rendering by merging only onto blank cells.
    // This guarantees data curves keep precedence wherever both map to the same terminal cell.
    if !threshold_overlay_datasets.is_empty() && chart_top <= chart_bottom {
        let threshold_chart = Chart::new(threshold_overlay_datasets)
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

        let mut threshold_buf = ratatui::buffer::Buffer::empty(chart_area);
        threshold_chart.render(chart_area, &mut threshold_buf);

        merge_overlay_buffer(frame, &threshold_buf, plot_bounds);
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
                                if is_blank_cell(cell) {
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

    if show_autogrid && chart_top <= chart_bottom {
        let plot_width = chart_right.saturating_sub(chart_left);
        let autogrid_time_ticks = calculate_time_grid_ticks(start, now, plot_width);
        let autogrid_datasets = build_autogrid_datasets(
            [start, now],
            y_bounds,
            &autogrid_time_ticks,
            &autogrid_value_ticks,
            plot_width,
            plot_height,
        );
        let autogrid_overlay_datasets: Vec<_> = autogrid_datasets
            .iter()
            .map(|dataset| {
                Dataset::default()
                    .name("")
                    .marker(ratatui::symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(app.autogrid_color))
                    .data(dataset)
            })
            .collect();

        let autogrid_chart = Chart::new(autogrid_overlay_datasets)
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

        let mut autogrid_buf = ratatui::buffer::Buffer::empty(chart_area);
        autogrid_chart.render(chart_area, &mut autogrid_buf);

        merge_overlay_buffer(frame, &autogrid_buf, plot_bounds);
        render_autogrid_time_labels(
            frame,
            plot_bounds,
            [start, now],
            &autogrid_time_ticks,
            time_range_secs,
            app.autogrid_color,
        );
    }

    render_intermediate_y_labels(
        frame,
        YLabelArea {
            left: chart_area.left(),
            width: y_max_width,
        },
        plot_bounds,
        y_bounds,
        &autogrid_value_ticks,
        &threshold_labels_info,
        app.autogrid_color,
    );

    // Render custom legend
    if legend_height > 0 {
        let legend = Paragraph::new(Line::from(legend_items)).wrap(Wrap { trim: true });
        frame.render_widget(legend, legend_area);
    }
}

#[derive(Debug, Clone, Copy)]
struct PlotBounds {
    left: u16,
    right: u16,
    top: u16,
    bottom: u16,
}

#[derive(Debug, Clone, Copy)]
struct YLabelArea {
    left: u16,
    width: u16,
}

fn merge_overlay_buffer(
    frame: &mut Frame,
    overlay_buf: &ratatui::buffer::Buffer,
    plot: PlotBounds,
) {
    let buf = frame.buffer_mut();
    for y in plot.top..=plot.bottom {
        for x in plot.left..plot.right {
            let Some(src_cell) = overlay_buf.cell((x, y)) else {
                continue;
            };
            if let Some(dst_cell) = buf.cell_mut((x, y)) {
                overlay_cell_if_blank(dst_cell, src_cell);
            }
        }
    }
}

fn is_blank_cell(cell: &ratatui::buffer::Cell) -> bool {
    cell.symbol().chars().all(char::is_whitespace)
}

fn overlay_cell_if_blank(dst: &mut ratatui::buffer::Cell, src: &ratatui::buffer::Cell) {
    if is_blank_cell(dst) && !is_blank_cell(src) {
        dst.set_symbol(src.symbol()).set_style(src.style());
    }
}

fn calculate_value_grid_ticks(y_bounds: [f64; 2], chart_height: u16) -> Vec<f64> {
    let min = y_bounds[0];
    let max = y_bounds[1];
    if !min.is_finite() || !max.is_finite() || max <= min || chart_height < 4 {
        return Vec::new();
    }

    let target_lines = (usize::from(chart_height) / 6).clamp(2, 4);
    let step = nice_grid_step(max - min, target_lines);
    if step <= 0.0 || !step.is_finite() {
        return Vec::new();
    }

    let mut ticks = Vec::new();
    let mut tick = (min / step).ceil() * step;
    while tick < max {
        if tick > min {
            ticks.push(tick);
        }
        tick += step;
    }
    ticks
}

fn nice_grid_step(range: f64, target_lines: usize) -> f64 {
    if range <= 0.0 || !range.is_finite() || target_lines == 0 {
        return 0.0;
    }

    let raw_step = range / target_lines as f64;
    let magnitude = 10_f64.powf(raw_step.log10().floor());
    let fraction = raw_step / magnitude;
    let nice_fraction = if fraction <= 1.0 {
        1.0
    } else if fraction <= 2.0 {
        2.0
    } else if fraction <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice_fraction * magnitude
}

fn calculate_time_grid_ticks(start: f64, end: f64, chart_width: u16) -> Vec<f64> {
    if !start.is_finite() || !end.is_finite() || end <= start || chart_width < 8 {
        return Vec::new();
    }

    let range = end - start;
    let mut step = base_time_grid_step(range);
    let max_ticks = (usize::from(chart_width) / 20).clamp(3, 8);
    while count_interior_ticks(start, end, step) > max_ticks {
        step = next_time_grid_step(step);
    }

    let mut ticks = Vec::new();
    let mut tick = (start / step).ceil() * step;
    while tick < end {
        if tick > start {
            ticks.push(tick);
        }
        tick += step;
    }
    ticks
}

fn base_time_grid_step(range: f64) -> f64 {
    const MINUTE: f64 = 60.0;
    const HOUR: f64 = 60.0 * MINUTE;
    const DAY: f64 = 24.0 * HOUR;

    if range <= 10.0 * MINUTE {
        MINUTE
    } else if range <= 30.0 * MINUTE {
        5.0 * MINUTE
    } else if range <= 90.0 * MINUTE {
        30.0 * MINUTE
    } else if range <= 3.0 * HOUR {
        HOUR
    } else if range <= 6.0 * HOUR {
        2.0 * HOUR
    } else if range <= 12.0 * HOUR {
        3.0 * HOUR
    } else if range <= DAY {
        6.0 * HOUR
    } else if range <= 2.0 * DAY {
        12.0 * HOUR
    } else {
        DAY
    }
}

fn next_time_grid_step(step: f64) -> f64 {
    const STEPS: [f64; 10] = [
        60.0, 300.0, 600.0, 1800.0, 3600.0, 7200.0, 10800.0, 21600.0, 43200.0, 86400.0,
    ];

    STEPS
        .iter()
        .copied()
        .find(|candidate| *candidate > step)
        .unwrap_or(step * 2.0)
}

fn count_interior_ticks(start: f64, end: f64, step: f64) -> usize {
    if step <= 0.0 {
        return 0;
    }

    let mut count = 0;
    let mut tick = (start / step).ceil() * step;
    while tick < end {
        if tick > start {
            count += 1;
        }
        tick += step;
    }
    count
}

fn build_autogrid_datasets(
    x_bounds: [f64; 2],
    y_bounds: [f64; 2],
    time_ticks: &[f64],
    value_ticks: &[f64],
    plot_width: u16,
    plot_height: u16,
) -> Vec<Vec<(f64, f64)>> {
    if x_bounds[1] <= x_bounds[0] || y_bounds[1] <= y_bounds[0] {
        return Vec::new();
    }

    let mut datasets = Vec::new();
    let vertical_samples = usize::from(plot_height).saturating_mul(4).max(2);
    let horizontal_samples = usize::from(plot_width).saturating_mul(2).max(2);

    for tick in time_ticks {
        if *tick <= x_bounds[0] || *tick >= x_bounds[1] {
            continue;
        }
        datasets.push(
            (0..=vertical_samples)
                .map(|i| {
                    let y = interpolate(y_bounds[0], y_bounds[1], i, vertical_samples);
                    (*tick, y)
                })
                .collect(),
        );
    }

    for tick in value_ticks {
        if *tick <= y_bounds[0] || *tick >= y_bounds[1] {
            continue;
        }
        datasets.push(
            (0..=horizontal_samples)
                .map(|i| {
                    let x = interpolate(x_bounds[0], x_bounds[1], i, horizontal_samples);
                    (x, *tick)
                })
                .collect(),
        );
    }
    datasets
}

fn interpolate(start: f64, end: f64, index: usize, total: usize) -> f64 {
    start + (end - start) * index as f64 / total as f64
}

fn y_label_width(
    axis_labels: &[Span<'_>],
    autogrid_ticks: &[f64],
    threshold_labels: &[(f64, Color)],
) -> u16 {
    let axis_width = axis_labels
        .iter()
        .map(|label| label.width() as u16)
        .max()
        .unwrap_or(0);
    let grid_width = autogrid_ticks
        .iter()
        .map(|tick| format_si(*tick).len() as u16)
        .max()
        .unwrap_or(0);
    let threshold_width = threshold_labels
        .iter()
        .map(|(tick, _)| format_si(*tick).len() as u16)
        .max()
        .unwrap_or(0);

    axis_width.max(grid_width).max(threshold_width)
}

fn render_intermediate_y_labels(
    frame: &mut Frame,
    label_area: YLabelArea,
    plot: PlotBounds,
    y_bounds: [f64; 2],
    autogrid_ticks: &[f64],
    threshold_labels: &[(f64, Color)],
    grid_color: Color,
) {
    if label_area.width == 0 {
        return;
    }

    for tick in autogrid_ticks {
        if let Some(y) = value_to_y_label_row(*tick, y_bounds, plot) {
            write_right_aligned_label(
                frame,
                label_area.left,
                y,
                label_area.width,
                &format_si(*tick),
                grid_color,
            );
        }
    }

    for (tick, color) in threshold_labels {
        if let Some(y) = value_to_y_label_row(*tick, y_bounds, plot) {
            write_right_aligned_label(
                frame,
                label_area.left,
                y,
                label_area.width,
                &format_si(*tick),
                *color,
            );
        }
    }
}

fn render_autogrid_time_labels(
    frame: &mut Frame,
    plot: PlotBounds,
    x_bounds: [f64; 2],
    ticks: &[f64],
    range_secs: f64,
    color: Color,
) {
    let y = plot.bottom.saturating_add(1);
    for tick in ticks {
        if let Some(x) = value_to_plot_x(*tick, x_bounds, plot) {
            write_centered_label(
                frame,
                x,
                y,
                plot.left,
                plot.right,
                &format_axis_time(*tick, range_secs),
                color,
            );
        }
    }
}

fn value_to_plot_y(value: f64, y_bounds: [f64; 2], plot: PlotBounds) -> Option<u16> {
    if value <= y_bounds[0] || value >= y_bounds[1] || y_bounds[1] <= y_bounds[0] {
        return None;
    }

    let height = plot.bottom.saturating_sub(plot.top) as f64;
    let ratio = (value - y_bounds[0]) / (y_bounds[1] - y_bounds[0]);
    let y_offset = (ratio * height).round() as u16;
    Some(plot.bottom.saturating_sub(y_offset))
}

fn value_to_y_label_row(value: f64, y_bounds: [f64; 2], plot: PlotBounds) -> Option<u16> {
    value_to_plot_y(value, y_bounds, plot)
}

fn value_to_plot_x(value: f64, x_bounds: [f64; 2], plot: PlotBounds) -> Option<u16> {
    if value <= x_bounds[0] || value >= x_bounds[1] || x_bounds[1] <= x_bounds[0] {
        return None;
    }

    let width = plot.right.saturating_sub(plot.left).saturating_sub(1) as f64;
    let ratio = (value - x_bounds[0]) / (x_bounds[1] - x_bounds[0]);
    Some(plot.left.saturating_add((ratio * width).round() as u16))
}

fn write_right_aligned_label(
    frame: &mut Frame,
    left: u16,
    y: u16,
    width: u16,
    label: &str,
    color: Color,
) {
    let label_width = label.chars().count() as u16;
    let x = left.saturating_add(width.saturating_sub(label_width));
    write_label(frame, x, y, label, color, false);
}

fn write_centered_label(
    frame: &mut Frame,
    center: u16,
    y: u16,
    min_x: u16,
    max_x: u16,
    label: &str,
    color: Color,
) {
    let label_width = label.chars().count() as u16;
    let Some(start_x) = centered_label_start(center, label_width, min_x, max_x) else {
        return;
    };

    let buf = frame.buffer_mut();
    for offset in 0..label_width {
        let Some(cell) = buf.cell((start_x.saturating_add(offset), y)) else {
            return;
        };
        if !is_blank_cell(cell) {
            return;
        }
    }

    let style = Style::default().fg(color);
    for (offset, ch) in label.chars().enumerate() {
        if let Some(cell) = buf.cell_mut((start_x.saturating_add(offset as u16), y)) {
            cell.set_char(ch).set_style(style);
        }
    }
}

fn centered_label_start(center: u16, label_width: u16, min_x: u16, max_x: u16) -> Option<u16> {
    if label_width == 0 || max_x <= min_x {
        return None;
    }

    let half_width = label_width / 2;
    // For even-length labels, bias one cell right so the visual midpoint better matches
    // the target chart column instead of consistently leaning left.
    let mut start_x = center.checked_sub(half_width)?;
    if label_width % 2 == 0 {
        start_x = start_x.saturating_add(1);
    }
    let end_x_exclusive = start_x.saturating_add(label_width);
    if start_x < min_x || end_x_exclusive > max_x {
        return None;
    }

    Some(start_x)
}

fn write_label(frame: &mut Frame, x: u16, y: u16, label: &str, color: Color, blank_only: bool) {
    let style = Style::default().fg(color);
    let buf = frame.buffer_mut();
    for (offset, ch) in label.chars().enumerate() {
        let Some(cell) = buf.cell_mut((x.saturating_add(offset as u16), y)) else {
            continue;
        };
        if !blank_only || is_blank_cell(cell) {
            cell.set_char(ch).set_style(style);
        }
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

fn calculate_y_bounds(p: &PanelState) -> [f64; 2] {
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

fn format_si(val: f64) -> String {
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

fn format_time(ts: f64) -> String {
    use chrono::TimeZone;
    if let Some(dt) = chrono::Utc.timestamp_opt(ts as i64, 0).single() {
        dt.format("%H:%M:%S").to_string()
    } else {
        format!("{}", ts)
    }
}

fn format_axis_time(ts: f64, range_secs: f64) -> String {
    use chrono::{TimeZone, Timelike};

    const DAY: f64 = 24.0 * 60.0 * 60.0;
    if range_secs < DAY {
        return format_time(ts);
    }

    let Some(dt) = chrono::Utc.timestamp_opt(ts as i64, 0).single() else {
        return format!("{}", ts);
    };

    if range_secs < 7.0 * DAY {
        if dt.hour() == 0 && dt.minute() == 0 && dt.second() == 0 {
            dt.format("%b %d").to_string()
        } else {
            dt.format("%b %d %Hh").to_string()
        }
    } else if range_secs < 90.0 * DAY {
        dt.format("%b %d").to_string()
    } else if range_secs < 730.0 * DAY {
        dt.format("%Y-%m").to_string()
    } else {
        dt.format("%Y").to_string()
    }
}

/// Generate a color from a string using hash-based approach.
/// Uses HSL color space to ensure visually distinct, vibrant colors.
fn get_hash_color(name: &str) -> Color {
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
            autogrid: None,
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
    fn test_is_blank_cell_empty() {
        let cell = ratatui::buffer::Cell::default();
        assert!(is_blank_cell(&cell));
    }

    #[test]
    fn test_is_blank_cell_filled() {
        let mut cell = ratatui::buffer::Cell::default();
        cell.set_char('x');
        assert!(!is_blank_cell(&cell));
    }

    #[test]
    fn test_overlay_cell_if_blank_copies_when_destination_is_empty() {
        let mut dst = ratatui::buffer::Cell::default();
        let mut src = ratatui::buffer::Cell::default();
        src.set_char('-').set_style(Style::default().fg(Color::Red));

        overlay_cell_if_blank(&mut dst, &src);

        assert_eq!(dst.symbol(), "-");
        assert_eq!(dst.style().fg, Some(Color::Red));
    }

    #[test]
    fn test_overlay_cell_if_blank_keeps_existing_destination_marker() {
        let mut dst = ratatui::buffer::Cell::default();
        dst.set_char('x')
            .set_style(Style::default().fg(Color::LightBlue));
        let mut src = ratatui::buffer::Cell::default();
        src.set_char('-').set_style(Style::default().fg(Color::Red));

        overlay_cell_if_blank(&mut dst, &src);

        assert_eq!(dst.symbol(), "x");
        assert_eq!(dst.style().fg, Some(Color::LightBlue));
    }

    #[test]
    fn test_value_to_plot_y_matches_grid_row() {
        let plot = PlotBounds {
            left: 0,
            right: 20,
            top: 10,
            bottom: 20,
        };

        assert_eq!(value_to_plot_y(10.0, [0.0, 20.0], plot), Some(15));
        assert_eq!(value_to_plot_y(5.0, [0.0, 20.0], plot), Some(17));
        assert_eq!(value_to_plot_y(15.0, [0.0, 20.0], plot), Some(12));
        assert_eq!(value_to_plot_y(0.0, [0.0, 20.0], plot), None);
        assert_eq!(value_to_plot_y(20.0, [0.0, 20.0], plot), None);
    }

    #[test]
    fn test_value_to_y_label_row_matches_grid_row() {
        let plot = PlotBounds {
            left: 0,
            right: 20,
            top: 10,
            bottom: 20,
        };

        assert_eq!(value_to_y_label_row(10.0, [0.0, 20.0], plot), Some(15));
        assert_eq!(value_to_y_label_row(5.0, [0.0, 20.0], plot), Some(17));
        assert_eq!(value_to_y_label_row(15.0, [0.0, 20.0], plot), Some(12));
        assert_eq!(value_to_y_label_row(0.0, [0.0, 20.0], plot), None);
        assert_eq!(value_to_y_label_row(20.0, [0.0, 20.0], plot), None);
    }

    #[test]
    fn test_value_to_plot_x_matches_grid_column() {
        let plot = PlotBounds {
            left: 10,
            right: 21,
            top: 0,
            bottom: 10,
        };

        assert_eq!(value_to_plot_x(5.0, [0.0, 10.0], plot), Some(15));
        assert_eq!(value_to_plot_x(0.0, [0.0, 10.0], plot), None);
        assert_eq!(value_to_plot_x(10.0, [0.0, 10.0], plot), None);
    }

    #[test]
    fn test_centered_label_start_returns_centered_position_when_it_fits() {
        assert_eq!(centered_label_start(50, 8, 10, 90), Some(47));
        assert_eq!(centered_label_start(50, 7, 10, 90), Some(47));
    }

    #[test]
    fn test_centered_label_start_skips_labels_that_would_be_clamped() {
        assert_eq!(centered_label_start(12, 8, 10, 90), None);
        assert_eq!(centered_label_start(88, 8, 10, 90), None);
    }

    #[test]
    fn test_calculate_value_grid_ticks_round_values() {
        let ticks = calculate_value_grid_ticks([329.0, 1287.0], 20);
        assert_eq!(ticks, vec![500.0, 1000.0]);
    }

    #[test]
    fn test_calculate_value_grid_ticks_excludes_boundaries() {
        let ticks = calculate_value_grid_ticks([0.0, 100.0], 20);
        assert!(!ticks.contains(&0.0));
        assert!(!ticks.contains(&100.0));
    }

    #[test]
    fn test_calculate_value_grid_ticks_invalid_ranges() {
        assert!(calculate_value_grid_ticks([1.0, 1.0], 20).is_empty());
        assert!(calculate_value_grid_ticks([2.0, 1.0], 20).is_empty());
        assert!(calculate_value_grid_ticks([f64::NAN, 1.0], 20).is_empty());
        assert!(calculate_value_grid_ticks([0.0, 1.0], 3).is_empty());
    }

    #[test]
    fn test_calculate_time_grid_ticks_two_hour_window() {
        let start = 41_820.0; // 11:37 UTC
        let end = start + 2.0 * 60.0 * 60.0;

        let ticks = calculate_time_grid_ticks(start, end, 80);

        assert_eq!(ticks, vec![43_200.0, 46_800.0]); // 12:00, 13:00 UTC
    }

    #[test]
    fn test_calculate_time_grid_ticks_one_hour_window() {
        let start = 44_520.0; // 12:22 UTC
        let end = start + 60.0 * 60.0;

        let ticks = calculate_time_grid_ticks(start, end, 80);

        assert_eq!(ticks, vec![45_000.0, 46_800.0]); // 12:30, 13:00 UTC
    }

    #[test]
    fn test_calculate_time_grid_ticks_five_minute_window() {
        let start = 43_335.0; // 12:02:15 UTC
        let end = start + 5.0 * 60.0;

        let ticks = calculate_time_grid_ticks(start, end, 120);

        assert_eq!(
            ticks,
            vec![43_380.0, 43_440.0, 43_500.0, 43_560.0, 43_620.0]
        );
    }

    #[test]
    fn test_format_axis_time_uses_time_for_short_ranges() {
        use chrono::TimeZone;

        let ts = chrono::Utc
            .with_ymd_and_hms(2026, 4, 30, 12, 34, 56)
            .single()
            .unwrap()
            .timestamp() as f64;

        assert_eq!(format_axis_time(ts, 60.0 * 60.0), "12:34:56");
    }

    #[test]
    fn test_format_axis_time_uses_date_for_multi_day_midnight_ticks() {
        use chrono::TimeZone;

        let ts = chrono::Utc
            .with_ymd_and_hms(2026, 4, 30, 0, 0, 0)
            .single()
            .unwrap()
            .timestamp() as f64;

        assert_eq!(format_axis_time(ts, 2.0 * 24.0 * 60.0 * 60.0), "Apr 30");
    }

    #[test]
    fn test_format_axis_time_keeps_hour_for_multi_day_non_midnight_ticks() {
        use chrono::TimeZone;

        let ts = chrono::Utc
            .with_ymd_and_hms(2026, 4, 30, 12, 0, 0)
            .single()
            .unwrap()
            .timestamp() as f64;

        assert_eq!(format_axis_time(ts, 2.0 * 24.0 * 60.0 * 60.0), "Apr 30 12h");
    }

    #[test]
    fn test_format_axis_time_scales_to_wider_ranges() {
        use chrono::TimeZone;

        let ts = chrono::Utc
            .with_ymd_and_hms(2026, 4, 30, 12, 0, 0)
            .single()
            .unwrap()
            .timestamp() as f64;
        let day = 24.0 * 60.0 * 60.0;

        assert_eq!(format_axis_time(ts, 14.0 * day), "Apr 30");
        assert_eq!(format_axis_time(ts, 120.0 * day), "2026-04");
        assert_eq!(format_axis_time(ts, 800.0 * day), "2026");
    }

    #[test]
    fn test_build_autogrid_datasets() {
        let datasets = build_autogrid_datasets([0.0, 10.0], [0.0, 10.0], &[5.0], &[5.0], 10, 5);

        assert_eq!(datasets.len(), 2);
        assert_eq!(datasets[0].first(), Some(&(5.0, 0.0)));
        assert_eq!(datasets[0].last(), Some(&(5.0, 10.0)));
        assert_eq!(datasets[0].len(), 21);
        assert_eq!(datasets[1].first(), Some(&(0.0, 5.0)));
        assert_eq!(datasets[1].last(), Some(&(10.0, 5.0)));
        assert_eq!(datasets[1].len(), 21);
    }
}
