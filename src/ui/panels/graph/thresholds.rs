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

use super::labels::PlotBounds;
use super::overlay::{is_blank_cell, overlay_cell_if_blank_or_weak_area_fill};
use crate::app::{PanelState, ThresholdMode};
use ratatui::{prelude::*, widgets::GraphType};

pub(super) struct ThresholdRenderData {
    pub(super) datasets: Vec<Vec<(f64, f64)>>,
    pub(super) labels: Vec<(f64, Color)>,
}

pub(super) fn prepare_thresholds(
    panel: &PanelState,
    marker_name: &str,
    x_bounds: [f64; 2],
) -> ThresholdRenderData {
    let mut datasets = Vec::new();
    let mut labels = Vec::new();

    let Some(thresholds) = &panel.thresholds else {
        return ThresholdRenderData { datasets, labels };
    };

    for step in thresholds.steps.iter().filter(|step| step.value.is_some()) {
        let value = step.value.unwrap();
        let threshold_value = match thresholds.mode {
            ThresholdMode::Absolute => value,
            ThresholdMode::Percentage => {
                let min = panel.min.unwrap_or(0.0);
                let max = panel.max.unwrap_or(100.0);
                min + (value / 100.0) * (max - min)
            }
        };

        datasets.push(threshold_dataset(
            marker_name,
            thresholds.style.as_deref(),
            x_bounds,
            threshold_value,
        ));
        labels.push((threshold_value, step.color));
    }

    ThresholdRenderData { datasets, labels }
}

pub(super) fn threshold_marker(marker_name: &str) -> (ratatui::symbols::Marker, GraphType) {
    match marker_name.to_lowercase().as_str() {
        "braille" => (ratatui::symbols::Marker::Braille, GraphType::Line),
        "block" => (ratatui::symbols::Marker::Block, GraphType::Line),
        "bar" => (ratatui::symbols::Marker::Bar, GraphType::Line),
        "half-block" => (ratatui::symbols::Marker::HalfBlock, GraphType::Line),
        "quadrant" => (ratatui::symbols::Marker::Quadrant, GraphType::Line),
        "sextant" => (ratatui::symbols::Marker::Sextant, GraphType::Line),
        "octant" => (ratatui::symbols::Marker::Octant, GraphType::Line),
        "dashed" | "dashed-braille" => (ratatui::symbols::Marker::Braille, GraphType::Scatter),
        "dashed-block" => (ratatui::symbols::Marker::Block, GraphType::Scatter),
        "dashed-bar" => (ratatui::symbols::Marker::Bar, GraphType::Scatter),
        "dashed-half-block" => (ratatui::symbols::Marker::HalfBlock, GraphType::Scatter),
        "dashed-quadrant" => (ratatui::symbols::Marker::Quadrant, GraphType::Scatter),
        "dashed-sextant" => (ratatui::symbols::Marker::Sextant, GraphType::Scatter),
        "dashed-octant" => (ratatui::symbols::Marker::Octant, GraphType::Scatter),
        "dashed-dot" => (ratatui::symbols::Marker::Dot, GraphType::Scatter),
        _ => (ratatui::symbols::Marker::Dot, GraphType::Line),
    }
}

pub(super) fn render_raw_threshold_lines(
    frame: &mut Frame,
    marker_name: &str,
    threshold_labels: &[(f64, Color)],
    y_bounds: [f64; 2],
    plot: PlotBounds,
    strong_data_buf: Option<&ratatui::buffer::Buffer>,
) {
    if !marker_name.ends_with("line") || y_bounds[1] <= y_bounds[0] {
        return;
    }

    let chart_height = plot.bottom.saturating_sub(plot.top) as f64;
    if chart_height <= 0.0 {
        return;
    }

    let is_dashed = marker_name.starts_with("dashed");
    let line_char = if is_dashed { '-' } else { '─' };
    let buf = frame.buffer_mut();

    for (threshold_value, color) in threshold_labels {
        if *threshold_value <= y_bounds[0] || *threshold_value >= y_bounds[1] {
            continue;
        }

        let ratio = (*threshold_value - y_bounds[0]) / (y_bounds[1] - y_bounds[0]);
        let y_offset = (ratio * chart_height).round() as u16;
        let y = plot.bottom.saturating_sub(y_offset);

        if y < plot.top || y > plot.bottom {
            continue;
        }

        for x in plot.left..plot.right {
            if is_dashed && x % 2 == 0 {
                continue;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                let mut line_cell = ratatui::buffer::Cell::default();
                line_cell
                    .set_char(line_char)
                    .set_style(Style::default().fg(*color));

                if let Some(strong_data_buf) = strong_data_buf {
                    if let Some(mask_cell) = strong_data_buf.cell((x, y)) {
                        overlay_cell_if_blank_or_weak_area_fill(cell, &line_cell, mask_cell);
                    }
                } else if is_blank_cell(cell) {
                    cell.set_symbol(line_cell.symbol())
                        .set_style(line_cell.style());
                }
            }
        }
    }
}

fn threshold_dataset(
    marker_name: &str,
    threshold_style: Option<&str>,
    x_bounds: [f64; 2],
    threshold_value: f64,
) -> Vec<(f64, f64)> {
    let [start, end] = x_bounds;
    if marker_name.starts_with("dashed") || threshold_style == Some("dashed") {
        let points_count = 15;
        let step_x = (end - start) / points_count as f64;
        return (0..=points_count)
            .map(|i| (start + (i as f64 * step_x), threshold_value))
            .collect();
    }

    vec![(start, threshold_value), (end, threshold_value)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn plot_bounds() -> PlotBounds {
        PlotBounds {
            left: 0,
            right: 5,
            top: 0,
            bottom: 4,
        }
    }

    #[test]
    fn test_raw_threshold_line_replaces_weak_area_fill_with_mask() {
        let mut terminal = Terminal::new(TestBackend::new(5, 5)).unwrap();
        let mask = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 5, 5));

        terminal
            .draw(|frame| {
                frame
                    .buffer_mut()
                    .cell_mut((1, 2))
                    .unwrap()
                    .set_char('⣿')
                    .set_style(Style::default().fg(Color::Blue));

                render_raw_threshold_lines(
                    frame,
                    "dashed-line",
                    &[(5.0, Color::Red)],
                    [0.0, 10.0],
                    plot_bounds(),
                    Some(&mask),
                );
            })
            .unwrap();

        let cell = terminal.backend().buffer().cell((1, 2)).unwrap();
        assert_eq!(cell.symbol(), "-");
        assert_eq!(cell.style().fg, Some(Color::Red));
    }

    #[test]
    fn test_raw_threshold_line_preserves_strong_data_with_mask() {
        let mut terminal = Terminal::new(TestBackend::new(5, 5)).unwrap();
        let mut mask = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 5, 5));
        mask.cell_mut((1, 2)).unwrap().set_char('⠉');

        terminal
            .draw(|frame| {
                frame
                    .buffer_mut()
                    .cell_mut((1, 2))
                    .unwrap()
                    .set_char('⣿')
                    .set_style(Style::default().fg(Color::Blue));

                render_raw_threshold_lines(
                    frame,
                    "dashed-line",
                    &[(5.0, Color::Red)],
                    [0.0, 10.0],
                    plot_bounds(),
                    Some(&mask),
                );
            })
            .unwrap();

        let cell = terminal.backend().buffer().cell((1, 2)).unwrap();
        assert_eq!(cell.symbol(), "⣿");
        assert_eq!(cell.style().fg, Some(Color::Blue));
    }
}
