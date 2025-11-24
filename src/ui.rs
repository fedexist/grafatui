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
        "Mode: {} | Prom: {} | range={} step={:?} refresh={} | panels={} (skipped {}) errors={} | keys: ↑/↓ scroll, r refresh, +/- range, q quit, ? debug:{}",
        mode_display,
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
            app.panels.get(0)
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

    // Render grid-backed panels
    for (i, p) in app.panels.iter().enumerate() {
        if let Some(g) = p.grid {
            if g.x < 0 || g.y < 0 || g.w <= 0 || g.h <= 0 {
                continue;
            }
            let x = area.x.saturating_add((g.x as u16).saturating_mul(cell_w));
            let y = area.y.saturating_add((g.y as u16).saturating_mul(cell_h));
            let w = (g.w as u16).saturating_mul(cell_w);
            let h = (g.h as u16).saturating_mul(cell_h);

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

    // Split inner area into chart and legend
    // If we have series, reserve space for legend
    let legend_height = if !p.series.is_empty() && inner_area.height > 5 {
        2
    } else {
        0
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(legend_height)])
        .split(inner_area);

    let chart_area = chunks[0];
    let legend_area = chunks[1];

    // Prepare datasets (without names for the chart itself to avoid built-in legend)
    let mut chart_datasets = Vec::new();
    let mut legend_items = Vec::new();

    // Declare cursor_dataset here to extend its lifetime
    let mut cursor_dataset = vec![];

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

        legend_items.push(Span::styled(format!("■ "), Style::default().fg(color)));
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

    // Calculate y_bounds once
    let y_bounds = calculate_y_bounds(p);

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

    // Determine x bounds from range window (unix seconds)
    // Use app.time_offset to shift the window
    let now = (chrono::Utc::now().timestamp() - app.time_offset.as_secs() as i64) as f64;
    let start = now - app.range.as_secs_f64();

    let x_labels = vec![
        Span::styled(format_time(start), Style::default().fg(theme.text)),
        Span::styled(format_time(now), Style::default().fg(theme.text)),
    ];

    let y_labels = vec![
        Span::styled(format_si(y_bounds[0]), Style::default().fg(theme.text)),
        Span::styled(format_si(y_bounds[1]), Style::default().fg(theme.text)),
    ];

    let chart = Chart::new(chart_datasets)
        // No block, as we rendered it outside
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
    // No legend position needed as we disabled names

    frame.render_widget(chart, chart_area);

    // Render custom legend
    if legend_height > 0 {
        let legend = Paragraph::new(Line::from(legend_items)).wrap(Wrap { trim: true });
        frame.render_widget(legend, legend_area);
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
}
