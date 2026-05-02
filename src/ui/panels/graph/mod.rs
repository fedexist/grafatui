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

mod autogrid;
mod bounds;
mod labels;
mod overlay;
mod thresholds;

use autogrid::{build_autogrid_datasets, calculate_time_grid_ticks, calculate_value_grid_ticks};
use labels::{
    PlotBounds, YLabelArea, render_autogrid_time_labels, render_intermediate_y_labels,
    y_label_width,
};
use overlay::merge_overlay_buffer;
use thresholds::{prepare_thresholds, render_raw_threshold_lines, threshold_marker};

use crate::app::{AppState, PanelState};
use crate::ui::format::{format_axis_time, format_si, get_hash_color};
use ratatui::{
    prelude::*,
    widgets::{Axis, Chart, Dataset, GraphType, Paragraph, Wrap},
};
use std::collections::HashMap;

pub(crate) use bounds::calculate_y_bounds;

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
    let threshold_data = prepare_thresholds(p, &app.threshold_marker, [start, now]);
    let mut threshold_overlay_datasets = Vec::new();

    if !app.threshold_marker.ends_with("line") {
        let (marker, graph_type) = threshold_marker(&app.threshold_marker);
        for (i, (_, color)) in threshold_data.labels.iter().enumerate() {
            threshold_overlay_datasets.push(
                Dataset::default()
                    .name("")
                    .marker(marker)
                    .graph_type(graph_type)
                    .style(Style::default().fg(*color))
                    .data(&threshold_data.datasets[i]),
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
    let y_max_width = y_label_width(&y_labels, &autogrid_value_ticks, &threshold_data.labels);

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

    render_raw_threshold_lines(
        frame,
        &app.threshold_marker,
        &threshold_data.labels,
        y_bounds,
        plot_bounds,
    );

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
        &threshold_data.labels,
        app.autogrid_color,
    );

    // Render custom legend
    if legend_height > 0 {
        let legend = Paragraph::new(Line::from(legend_items)).wrap(Wrap { trim: true });
        frame.render_widget(legend, legend_area);
    }
}
