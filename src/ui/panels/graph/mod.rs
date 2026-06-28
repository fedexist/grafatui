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

mod autogrid;
mod bounds;
mod labels;
mod overlay;
mod thresholds;

use autogrid::{build_autogrid_datasets, calculate_time_grid_ticks, calculate_value_grid_ticks};
use labels::{
    PlotBounds, YLabelArea, YLabelContext, render_autogrid_time_labels,
    render_intermediate_y_labels, y_label_width,
};
use overlay::{merge_overlay_buffer, merge_overlay_buffer_preserving_data};
use thresholds::{prepare_thresholds, render_raw_threshold_lines, threshold_marker};

use crate::app::{AppState, PanelState};
use crate::ui::format::{format_axis_time, get_hash_color};
use ratatui::{
    prelude::*,
    widgets::{Axis, Chart, Dataset, GraphType, Paragraph, Wrap},
};
use std::collections::HashMap;

pub(crate) use bounds::calculate_y_bounds;

fn graph_type_for_draw_style(draw_style: crate::app::GraphDrawStyle) -> GraphType {
    match draw_style {
        crate::app::GraphDrawStyle::Line => GraphType::Line,
        crate::app::GraphDrawStyle::Points => GraphType::Scatter,
        crate::app::GraphDrawStyle::Bars => GraphType::Bar,
    }
}

fn should_overlay_points(options: &crate::app::GraphOptions) -> bool {
    options.show_points == crate::app::GraphPointMode::Always
        && options.draw_style != crate::app::GraphDrawStyle::Points
}

fn area_fill_baseline(y_bounds: [f64; 2]) -> f64 {
    if y_bounds[0] <= 0.0 && y_bounds[1] >= 0.0 {
        0.0
    } else {
        y_bounds[0]
    }
}

fn is_y_axis_hidden(options: &crate::app::GraphOptions) -> bool {
    options.axis_placement == crate::app::GraphAxisPlacement::Hidden
}

fn chart_plot_left(
    chart_area: Rect,
    y_label_width: u16,
    x_labels: &[Span<'_>],
    has_y_axis_labels: bool,
) -> u16 {
    let first_x_label_width = x_labels
        .first()
        .map(|label| label.width() as u16)
        .unwrap_or_default();
    let y_axis_offset = u16::from(has_y_axis_labels);
    let x_label_gutter = first_x_label_width.saturating_sub(y_axis_offset);
    let labels_left_of_y_axis = if has_y_axis_labels {
        y_label_width.max(x_label_gutter)
    } else {
        x_label_gutter
    }
    .min(chart_area.width / 3);
    let gutter = labels_left_of_y_axis + y_axis_offset;

    chart_area.left() + gutter
}

fn chart_y_label_width(labels: &[Span<'_>]) -> u16 {
    labels
        .iter()
        .map(|label| label.width() as u16)
        .max()
        .unwrap_or_default()
}

fn point_to_braille_cell(
    x: f64,
    y: f64,
    x_bounds: [f64; 2],
    y_bounds: [f64; 2],
    plot: PlotBounds,
) -> Option<(u16, u16)> {
    if !x.is_finite()
        || !y.is_finite()
        || !x_bounds[0].is_finite()
        || !x_bounds[1].is_finite()
        || !y_bounds[0].is_finite()
        || !y_bounds[1].is_finite()
        || x < x_bounds[0]
        || x > x_bounds[1]
        || y < y_bounds[0]
        || y > y_bounds[1]
        || x_bounds[1] <= x_bounds[0]
        || y_bounds[1] <= y_bounds[0]
        || plot.right <= plot.left
        || plot.bottom < plot.top
    {
        return None;
    }

    let plot_width = plot.right.saturating_sub(plot.left);
    let plot_height = plot.bottom.saturating_sub(plot.top).saturating_add(1);
    let x_resolution = f64::from(plot_width) * 2.0;
    let y_resolution = f64::from(plot_height) * 4.0;

    let braille_x =
        ((x - x_bounds[0]) * (x_resolution - 1.0) / (x_bounds[1] - x_bounds[0])).round() as u16;
    let braille_y =
        ((y_bounds[1] - y) * (y_resolution - 1.0) / (y_bounds[1] - y_bounds[0])).round() as u16;

    let cell_x = plot.left.saturating_add(braille_x / 2);
    let cell_y = plot.top.saturating_add(braille_y / 4);
    (cell_x < plot.right && cell_y <= plot.bottom).then_some((cell_x, cell_y))
}

fn render_forced_point_markers(
    frame: &mut Frame,
    markers: &[(f64, f64, Color)],
    x_bounds: [f64; 2],
    y_bounds: [f64; 2],
    plot: PlotBounds,
) {
    let buf = frame.buffer_mut();
    for (x, y, color) in markers {
        let Some((cell_x, cell_y)) = point_to_braille_cell(*x, *y, x_bounds, y_bounds, plot) else {
            continue;
        };
        if let Some(cell) = buf.cell_mut((cell_x, cell_y)) {
            cell.set_symbol(ratatui::symbols::DOT)
                .set_style(Style::default().fg(*color));
        }
    }
}

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
    let graph_options = p.graph_options();
    let hide_y_axis = is_y_axis_hidden(&graph_options);

    // Prepare datasets (without names for the chart itself to avoid built-in legend)
    let mut chart_datasets = Vec::new();
    let mut strong_data_datasets = Vec::new();
    let mut legend_items = Vec::new();
    let mut forced_point_markers = Vec::new();

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
            name.push_str(&format!(" ({})", p.display.format_number(*val)));
        } else if let Some(val) = s.value {
            name.push_str(&format!(" ({})", p.display.format_number(val)));
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
        let mut dataset = Dataset::default()
            .name("")
            .marker(ratatui::symbols::Marker::Braille)
            .graph_type(graph_type_for_draw_style(graph_options.draw_style))
            .style(Style::default().fg(color))
            .data(data);

        let is_area_filled = graph_options.fill_opacity.unwrap_or(0) > 0
            && graph_options.draw_style == crate::app::GraphDrawStyle::Line;

        if is_area_filled {
            strong_data_datasets.push(
                Dataset::default()
                    .name("")
                    .marker(ratatui::symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(color))
                    .data(data),
            );

            dataset = dataset
                .graph_type(GraphType::Area)
                .fill_to_y(area_fill_baseline(y_bounds));
        }

        chart_datasets.push(dataset);

        if should_overlay_points(&graph_options) {
            forced_point_markers.extend(data.iter().map(|(x, y)| (*x, *y, color)));
        }
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

        if !strong_data_datasets.is_empty() {
            strong_data_datasets.push(
                Dataset::default()
                    .name("")
                    .marker(ratatui::symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(Color::White))
                    .data(&cursor_dataset),
            );
        }
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

    if !hide_y_axis {
        y_labels[0] = Span::styled(
            p.display.format_number(y_bounds[0]),
            Style::default().fg(theme.text),
        );
        y_labels[y_axis_height - 1] = Span::styled(
            p.display.format_number(y_bounds[1]),
            Style::default().fg(theme.text),
        );
    }

    // Evaluate y_max_width before moving y_labels into Chart block
    let y_max_width = if hide_y_axis {
        0
    } else {
        y_label_width(
            &y_labels,
            &autogrid_value_ticks,
            &threshold_data.labels,
            &p.display,
        )
    };
    let chart_y_labels = if hide_y_axis {
        Vec::new()
    } else {
        y_labels.clone()
    };
    let chart_y_label_width = chart_y_label_width(&chart_y_labels);

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
                .labels(chart_y_labels.clone()),
        );
    // No legend position needed as we disabled names

    frame.render_widget(chart, chart_area);

    let strong_data_buf = if strong_data_datasets.is_empty() {
        None
    } else {
        let strong_data_chart = Chart::new(strong_data_datasets)
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
                    .labels(chart_y_labels.clone()),
            );
        let mut buf = ratatui::buffer::Buffer::empty(chart_area);
        strong_data_chart.render(chart_area, &mut buf);
        Some(buf)
    };

    let chart_left = chart_plot_left(
        chart_area,
        chart_y_label_width,
        &x_labels,
        !chart_y_labels.is_empty(),
    );
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
                    .labels(chart_y_labels.clone()),
            );

        let mut threshold_buf = ratatui::buffer::Buffer::empty(chart_area);
        threshold_chart.render(chart_area, &mut threshold_buf);

        if let Some(strong_data_buf) = strong_data_buf.as_ref() {
            merge_overlay_buffer_preserving_data(
                frame,
                &threshold_buf,
                strong_data_buf,
                plot_bounds,
            );
        } else {
            merge_overlay_buffer(frame, &threshold_buf, plot_bounds);
        }
    }

    render_raw_threshold_lines(
        frame,
        &app.threshold_marker,
        &threshold_data.labels,
        y_bounds,
        plot_bounds,
        strong_data_buf.as_ref(),
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
                    .labels(chart_y_labels),
            );

        let mut autogrid_buf = ratatui::buffer::Buffer::empty(chart_area);
        autogrid_chart.render(chart_area, &mut autogrid_buf);

        // Autogrid is a background layer: it may fill empty plot cells, but it
        // should not cut through area fills or other rendered data.
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

    if !hide_y_axis {
        render_intermediate_y_labels(
            frame,
            YLabelArea {
                left: chart_area.left(),
                width: y_max_width,
            },
            plot_bounds,
            YLabelContext {
                y_bounds,
                autogrid_ticks: &autogrid_value_ticks,
                threshold_labels: &threshold_data.labels,
                display: &p.display,
                color: app.autogrid_color,
            },
        );
    }

    render_forced_point_markers(
        frame,
        &forced_point_markers,
        [start, now],
        y_bounds,
        plot_bounds,
    );

    // Render custom legend
    if legend_height > 0 {
        let legend = Paragraph::new(Line::from(legend_items)).wrap(Wrap { trim: true });
        frame.render_widget(legend, legend_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{
        GraphAxisPlacement, GraphDrawStyle, GraphOptions, GraphPointMode, GraphStackingMode,
        PanelOptions, PanelType, QueryMode, SeriesView, YAxisMode,
    };
    use crate::export::ExportOptions;
    use crate::theme::Theme;
    use ratatui::{Terminal, backend::TestBackend};
    use std::time::Duration;

    #[test]
    fn test_graph_type_for_draw_style() {
        assert_eq!(
            graph_type_for_draw_style(GraphDrawStyle::Line),
            GraphType::Line
        );
        assert_eq!(
            graph_type_for_draw_style(GraphDrawStyle::Points),
            GraphType::Scatter
        );
        assert_eq!(
            graph_type_for_draw_style(GraphDrawStyle::Bars),
            GraphType::Bar
        );
    }

    #[test]
    fn test_should_overlay_points() {
        let mut options = GraphOptions::default();
        options.draw_style = GraphDrawStyle::Line;
        options.show_points = GraphPointMode::Auto;
        assert!(!should_overlay_points(&options));

        options.show_points = GraphPointMode::Always;
        assert!(should_overlay_points(&options));

        options.draw_style = GraphDrawStyle::Points;
        assert!(!should_overlay_points(&options));

        options.draw_style = GraphDrawStyle::Bars;
        options.show_points = GraphPointMode::Always;
        assert!(should_overlay_points(&options));
    }

    #[test]
    fn test_area_fill_baseline_prefers_zero_when_visible() {
        assert_eq!(area_fill_baseline([-10.0, 20.0]), 0.0);
        assert_eq!(area_fill_baseline([5.0, 20.0]), 5.0);
        assert_eq!(area_fill_baseline([-20.0, -5.0]), -20.0);
    }

    #[test]
    fn test_hidden_axis_flag() {
        let visible = GraphOptions::default();
        assert!(!is_y_axis_hidden(&visible));

        let hidden = GraphOptions {
            axis_placement: GraphAxisPlacement::Hidden,
            draw_style: GraphDrawStyle::Line,
            show_points: GraphPointMode::Auto,
            fill_opacity: None,
            line_interpolation: Some("smooth".to_string()),
            stacking: GraphStackingMode::Normal,
        };
        assert!(is_y_axis_hidden(&hidden));
    }

    #[test]
    fn test_chart_plot_left_visible_axis_uses_y_label_width_when_dominant() {
        let chart_area = Rect::new(10, 0, 90, 20);
        let x_labels = vec![Span::raw("abc")];

        assert_eq!(chart_plot_left(chart_area, 6, &x_labels, true), 17);
    }

    #[test]
    fn test_chart_plot_left_visible_axis_uses_first_x_label_when_dominant() {
        let chart_area = Rect::new(10, 0, 90, 20);
        let x_labels = vec![Span::raw("long-start-label")];

        assert_eq!(chart_plot_left(chart_area, 6, &x_labels, true), 26);
    }

    #[test]
    fn test_chart_plot_left_visible_axis_clamps_gutter() {
        let chart_area = Rect::new(10, 0, 30, 20);
        let x_labels = vec![Span::raw("very-long-start-label")];

        assert_eq!(chart_plot_left(chart_area, 2, &x_labels, true), 21);
    }

    #[test]
    fn test_chart_plot_left_hidden_axis_uses_first_x_label_gutter() {
        let chart_area = Rect::new(10, 0, 90, 20);
        let x_labels = vec![Span::raw("long-start-label")];

        assert_eq!(chart_plot_left(chart_area, 0, &x_labels, false), 26);
    }

    #[test]
    fn test_chart_plot_left_hidden_axis_clamps_first_x_label_gutter() {
        let chart_area = Rect::new(10, 0, 30, 20);
        let x_labels = vec![Span::raw("very-long-start-label")];

        assert_eq!(chart_plot_left(chart_area, 0, &x_labels, false), 20);
    }

    #[test]
    fn test_chart_plot_left_ignores_custom_label_width() {
        let chart_area = Rect::new(10, 0, 90, 20);
        let x_labels = vec![Span::raw("abc")];
        let chart_y_labels = vec![Span::raw("0"), Span::raw("9")];
        let custom_label_width = 30;
        let chart_y_label_width = chart_y_label_width(&chart_y_labels);

        assert_eq!(
            chart_plot_left(
                chart_area,
                chart_y_label_width,
                &x_labels,
                !chart_y_labels.is_empty(),
            ),
            13
        );
        assert_ne!(
            chart_plot_left(
                chart_area,
                custom_label_width,
                &x_labels,
                !chart_y_labels.is_empty(),
            ),
            13
        );
    }

    fn area_fill_panel() -> PanelState {
        PanelState {
            title: "area".to_string(),
            exprs: vec![],
            legends: vec![],
            query_modes: vec![QueryMode::Range],
            series: vec![SeriesView {
                name: "filled".to_string(),
                value: Some(8.0),
                points: vec![(0.0, 8.0), (50.0, 8.0), (100.0, 8.0)],
                visible: true,
            }],
            last_error: None,
            last_url: None,
            last_samples: 3,
            grid: None,
            y_axis_mode: YAxisMode::Auto,
            panel_type: PanelType::Graph,
            thresholds: None,
            min: Some(0.0),
            max: Some(10.0),
            autogrid: Some(true),
            display: crate::ui::DisplayFormat::default(),
            options: PanelOptions::Graph(GraphOptions {
                draw_style: GraphDrawStyle::Line,
                show_points: GraphPointMode::Never,
                fill_opacity: Some(30),
                axis_placement: GraphAxisPlacement::Visible,
                line_interpolation: None,
                stacking: GraphStackingMode::Off,
            }),
        }
    }

    fn area_fill_app(panel: PanelState) -> AppState {
        let mut app = AppState::new(
            crate::prom::PromClient::new("http://localhost:9090".to_string()),
            Duration::from_secs(100),
            Duration::from_secs(5),
            Duration::from_secs(1),
            "test".to_string(),
            vec![panel],
            0,
            Theme::default(),
            "dashed-line".to_string(),
            ExportOptions::default(),
        );
        app.view_end_ts = 100;
        app.autogrid_color = Color::Red;
        app
    }

    #[test]
    fn test_area_fill_keeps_precedence_over_autogrid() {
        let app = area_fill_app(area_fill_panel());
        let panel = &app.panels[0];
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();

        terminal
            .draw(|frame| {
                render_graph_panel(frame, Rect::new(0, 0, 80, 20), panel, &app, None);
            })
            .unwrap();

        let y_bounds = calculate_y_bounds(panel);
        let chart_area = Rect::new(0, 0, 80, 18);
        let x_labels = vec![Span::raw("00:00:00"), Span::raw("00:01:40")];
        let chart_y_labels = vec![Span::raw("0"), Span::raw("10")];
        let plot = PlotBounds {
            left: chart_plot_left(
                chart_area,
                chart_y_label_width(&chart_y_labels),
                &x_labels,
                true,
            ),
            right: chart_area.right(),
            top: chart_area.top(),
            bottom: chart_area.bottom().saturating_sub(2),
        };
        let plot_height = plot.bottom.saturating_sub(plot.top) as f64;
        let grid_ratio = (5.0 - y_bounds[0]) / (y_bounds[1] - y_bounds[0]);
        let grid_y = plot
            .bottom
            .saturating_sub((grid_ratio * plot_height).round() as u16);

        let grid_colored_cells_inside_fill = (plot.left..plot.right)
            .filter(|x| {
                terminal
                    .backend()
                    .buffer()
                    .cell((*x, grid_y))
                    .is_some_and(|cell| cell.style().fg == Some(Color::Red))
            })
            .count();

        assert_eq!(grid_colored_cells_inside_fill, 0);
    }

    #[test]
    fn test_line_forced_points_are_visible_and_use_line_marker_cells() {
        let mut panel = area_fill_panel();
        panel.series[0].points = vec![(25.0, 2.0), (50.0, 8.0), (75.0, 4.0)];
        panel.options = PanelOptions::Graph(GraphOptions {
            draw_style: GraphDrawStyle::Line,
            show_points: GraphPointMode::Always,
            fill_opacity: None,
            axis_placement: GraphAxisPlacement::Visible,
            line_interpolation: None,
            stacking: GraphStackingMode::Off,
        });
        let app = area_fill_app(panel);
        let panel = &app.panels[0];
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();

        terminal
            .draw(|frame| {
                render_graph_panel(frame, Rect::new(0, 0, 80, 20), panel, &app, None);
            })
            .unwrap();

        let y_bounds = calculate_y_bounds(panel);
        let chart_area = Rect::new(0, 0, 80, 18);
        let x_labels = vec![Span::raw("00:00:00"), Span::raw("00:01:40")];
        let chart_y_labels = vec![Span::raw("0"), Span::raw("10")];
        let plot = PlotBounds {
            left: chart_plot_left(
                chart_area,
                chart_y_label_width(&chart_y_labels),
                &x_labels,
                true,
            ),
            right: chart_area.right(),
            top: chart_area.top(),
            bottom: chart_area.bottom().saturating_sub(2),
        };
        let expected_cells: std::collections::HashSet<_> = panel.series[0]
            .points
            .iter()
            .filter_map(|(x, y)| point_to_braille_cell(*x, *y, [0.0, 100.0], y_bounds, plot))
            .collect();
        let visible_point_cells: std::collections::HashSet<_> = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| {
                (cell.symbol() == "•").then_some(((index % 80) as u16, (index / 80) as u16))
            })
            .collect();

        assert!(!visible_point_cells.is_empty());
        assert_eq!(visible_point_cells, expected_cells);
    }
}
