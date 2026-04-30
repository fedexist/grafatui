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

use crate::app::{AppState, PanelState, ThresholdMode};
use crate::ui::format::{calculate_y_bounds, format_axis_time, format_si, get_hash_color};
use ratatui::{
    prelude::*,
    widgets::{Axis, Chart, Dataset, GraphType, Paragraph, Wrap},
};
use std::collections::HashMap;

pub(super) fn render_graph_panel(
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
                ThresholdMode::Absolute => val,
                ThresholdMode::Percentage => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Style;

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
