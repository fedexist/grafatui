use crate::app::{AppState, PanelState};
use chrono::Utc;
use humantime::format_duration;
use ratatui::{
    prelude::*,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph, Wrap},
};

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
    let title = format!(
        "{} — range={} step={:?}  panels={}  (r to refresh, +/- range, q quit)",
        app.title,
        format_duration(app.range),
        app.step,
        app.panels.len()
    );
    let title_block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(title).alignment(Alignment::Center));
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

fn render_panel(frame: &mut Frame, area: Rect, p: &PanelState, app: &AppState) {
    if let Some(err) = &p.last_error {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!("{} — ERROR", p.title));
        let para = Paragraph::new(err.clone())
            .block(block)
            .wrap(Wrap { trim: true });
        frame.render_widget(para, area);
        return;
    }

    // Determine x bounds from range window (unix seconds)
    let end = Utc::now().timestamp() as f64;
    let start = end - app.range.as_secs_f64();

    let (ymin, ymax) = calculate_y_bounds(&p.series);

    let datasets: Vec<Dataset> = p
        .series
        .iter()
        .map(|s| {
            Dataset::default()
                .name(s.legend.clone())
                .marker(ratatui::symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .data(&s.points)
        })
        .collect();

    let mid = (start + end) / 2.0;
    let xlabels = vec![
        ts_label(start as i64),
        ts_label(mid as i64),
        ts_label(end as i64),
    ];
    let ylabels = vec![
        format!("{:.3}", ymin),
        format!("{:.3}", (ymin + ymax) / 2.0),
        format!("{:.3}", ymax),
    ];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(p.title.clone()),
        )
        .x_axis(
            Axis::default()
                .bounds([start, end])
                .labels(xlabels.into_iter().map(Line::from).collect::<Vec<_>>()),
        )
        .y_axis(
            Axis::default()
                .bounds([ymin, ymax])
                .labels(ylabels.into_iter().map(Line::from).collect::<Vec<_>>()),
        );

    frame.render_widget(chart, area);
}

fn ts_label(ts: i64) -> String {
    use chrono::DateTime;
    DateTime::from_timestamp(ts, 0)
        .unwrap_or_default()
        .format("%H:%M:%S")
        .to_string()
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

    for (p, rect) in left.iter().zip(left_chunks.iter()) {
        render_panel(frame, *rect, *p, app);
    }
    for (p, rect) in right.iter().zip(right_chunks.iter()) {
        render_panel(frame, *rect, *p, app);
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
            render_panel(frame, rect, p, app);
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

fn calculate_y_bounds(series: &[crate::app::SeriesView]) -> (f64, f64) {
    let mut ymin = f64::INFINITY;
    let mut ymax = f64::NEG_INFINITY;
    for s in series {
        for &(_, y) in &s.points {
            ymin = ymin.min(y);
            ymax = ymax.max(y);
        }
    }
    if !ymin.is_finite() || !ymax.is_finite() {
        ymin = 0.0;
        ymax = 1.0;
    }
    if (ymax - ymin).abs() < 1e-9 {
        ymax = ymin + 1.0;
    }
    (ymin, ymax)
}
