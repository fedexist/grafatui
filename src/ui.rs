use crate::app::{AppState, PanelState};
use humantime::format_duration;
use ratatui::{
    prelude::*,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph, Wrap},
};

/// Renders the entire application UI into the given frame.
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
    let has_grid = app.panels.iter().any(|p| p.grid.is_some());
    if has_grid {
        render_grafana_grid(frame, area, app);
    } else {
        render_two_column_flow(frame, area, app);
    }

    // Footer / Status bar
    let errors = app.panels.iter().filter(|p| p.last_error.is_some()).count();
    let summary = format!(
        "Prom: {} | range={} step={:?} refresh={} | panels={} (skipped {}) errors={} | keys: ↑/↓ scroll, r refresh, +/- range, q quit, ? debug:{}",
        app.prometheus.base,
        format_duration(app.range),
        app.step,
        format_duration(app.refresh_every),
        app.panels.len(),
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

    let footer = Paragraph::new(format!("{}\n{}", summary, detail)).wrap(Wrap { trim: true });
    frame.render_widget(footer, chunks[2]);
}

fn render_panel(frame: &mut Frame, area: Rect, p: &PanelState, app: &AppState, is_selected: bool) {
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

    let mut datasets = Vec::new();
    let use_hash_colors = p.series.len() > theme.palette.len();

    for (i, s) in p.series.iter().enumerate() {
        let color = if use_hash_colors {
            // Hash-based color assignment for many series
            get_hash_color(&s.name)
        } else {
            // Sequential palette assignment for few series
            theme.palette[i % theme.palette.len()]
        };

        let data = if s.visible { s.points.as_slice() } else { &[] };
        let mut name = s.name.clone();
        if let Some(val) = s.value {
            name.push_str(&format!(" ({})", format_si(val)));
        }
        datasets.push(
            Dataset::default()
                .name(name)
                .marker(ratatui::symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(color))
                .data(data),
        );
    }

    // Determine x bounds from range window (unix seconds)
    let now = chrono::Utc::now().timestamp() as f64;
    let start = now - app.range.as_secs_f64();

    let x_labels = vec![
        Span::styled(format_time(start), Style::default().fg(theme.text)),
        Span::styled(format_time(now), Style::default().fg(theme.text)),
    ];

    let y_bounds = calculate_y_bounds(p);
    let y_labels = vec![
        Span::styled(format_si(y_bounds[0]), Style::default().fg(theme.text)),
        Span::styled(format_si(y_bounds[1]), Style::default().fg(theme.text)),
    ];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(Span::styled(
                    p.title.clone(),
                    Style::default().fg(theme.title),
                )),
        )
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

    frame.render_widget(chart, area);

    // Custom Legend Rendering (since Chart widget filters out data, it might also hide legend items?
    // Actually Chart widget shows legend for datasets provided. If we filter datasets, legend is gone.
    // So we need to render a separate legend or accept that hidden series disappear from legend.
    // User requirement: "Dim or hide legend text for invisible series".
    // Let's try to keep them in legend but dimmed.
    // BUT Chart widget doesn't support "dimmed legend for missing dataset".
    // So we will just let them disappear for now, or we can pass empty data for hidden series?
    // If we pass empty data, the line won't be drawn, but legend will show.
    // Let's try passing empty data for hidden series.

    // REVERTING previous change to filter datasets. Instead, we map hidden series to empty data.
}

fn render_two_column_flow(frame: &mut Frame, area: Rect, app: &AppState) {
    let panels: Vec<&PanelState> = app.panels.iter().collect();
    render_panel_slice_two_column(frame, area, &panels, app);
}

fn render_panel_slice_two_column(
    frame: &mut Frame,
    area: Rect,
    panels: &[&PanelState],
    app: &AppState,
) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let panel_height = 12u16;
    // If we have many panels, we might need to scroll.
    // The app.vertical_scroll applies to the whole view.
    // If we are in "extras" mode (mixed grid + list), scrolling might be tricky if we don't separate it.
    // For now, let's apply scroll only if we are in the main two-column mode (all panels).
    // But since we reused this for extras, we might want to just render them all or handle scrolling there too.
    // Let's keep it simple: use the same scroll offset for now, but clamped to the slice length.

    let rows_fit = (area.height / panel_height).saturating_mul(2).max(1) as usize;
    let start = app
        .vertical_scroll
        .min(panels.len().saturating_sub(rows_fit));
    let end = (start + rows_fit).min(panels.len());

    let visible = &panels[start..end];
    let mut left = Vec::new();
    let mut right = Vec::new();
    for (i, p) in visible.iter().enumerate() {
        if i % 2 == 0 {
            left.push(p);
        } else {
            right.push(p);
        }
    }

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(panel_height); left.len()])
        .split(cols[0]);
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(panel_height); right.len()])
        .split(cols[1]);

    // Auto-scroll to keep selected panel in view
    // We need to ensure app.selected_panel is within [start, end).
    // This logic belongs in app.rs or we calculate start based on selected_panel here.
    // For simplicity, let's override start based on selected_panel if we are in 2-col mode.
    // But wait, render_panel_slice_two_column is generic.
    // Let's just use the passed slice and assume caller handles scrolling?
    // Actually, the previous logic used app.vertical_scroll.
    // Let's change it: if selected_panel is in the list, ensure it's visible.

    // NOTE: This function is used for both main list and extras.
    // We need to know the global index of the panel to check selection.
    // But we only have &PanelState. We can compare pointers or titles? Titles might not be unique.
    // Better: pass the index offset.

    // For now, let's just update the call sites.
    // Wait, we need to update render_panel signature first (done above).
    // Now we need to update the calls.

    for (p, rect) in left.iter().zip(left_chunks.iter()) {
        let p_ref: &PanelState = *p;
        let is_selected = app
            .panels
            .iter()
            .position(|x| {
                let x_ref: &PanelState = x;
                std::ptr::eq(x_ref, p_ref)
            })
            .unwrap_or(usize::MAX)
            == app.selected_panel;
        render_panel(frame, *rect, *p, app, is_selected);
    }
    for (p, rect) in right.iter().zip(right_chunks.iter()) {
        let p_ref: &PanelState = *p;
        let is_selected = app
            .panels
            .iter()
            .position(|x| {
                let x_ref: &PanelState = x;
                std::ptr::eq(x_ref, p_ref)
            })
            .unwrap_or(usize::MAX)
            == app.selected_panel;
        render_panel(frame, *rect, *p, app, is_selected);
    }
}

fn render_grafana_grid(frame: &mut Frame, area: Rect, app: &AppState) {
    // Grafana uses a 24-column grid; y/h units are arbitrary grid rows.
    let grid_cols: u16 = 24;
    let cell_w = std::cmp::max(1, area.width / grid_cols);
    // Heuristic: choose a usable cell height from terminal height (min 3 rows per h-unit)
    let cell_h = std::cmp::max(3, area.height / 24);

    // Render grid-backed panels
    let mut rendered_any = false;
    for p in app.panels.iter().filter(|p| p.grid.is_some()) {
        let g = p.grid.unwrap();
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
            // need some space to draw axes
            let is_selected = app
                .panels
                .iter()
                .position(|x| std::ptr::eq(x, p))
                .unwrap_or(usize::MAX)
                == app.selected_panel;
            render_panel(frame, rect, p, app, is_selected);
            rendered_any = true;
        }
    }

    // Any panel without grid gets stacked at the bottom (fallback)
    let extras: Vec<&PanelState> = app.panels.iter().filter(|p| p.grid.is_none()).collect();
    if !extras.is_empty() {
        // Place extras in a vertical stack under the grid.
        // We need to calculate where the grid ended.
        // A simple heuristic is to find the max Y+H of any grid panel.
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

            // Reuse the two-column logic for these extras
            // We need to adapt render_two_column_flow to take a slice of panels,
            // but it currently takes the whole app.
            // Let's refactor render_two_column_flow to take a slice of panels.
            render_panel_slice_two_column(frame, extras_area, &extras, app);
            rendered_any = true;
        }
    }

    if !rendered_any {
        let hint = Paragraph::new("Not enough space for Grafana grid; enlarge terminal.")
            .block(Block::default().borders(Borders::ALL).title("Layout"));
        frame.render_widget(hint, area);
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
