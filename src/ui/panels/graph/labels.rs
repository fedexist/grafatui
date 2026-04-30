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

use super::overlay::is_blank_cell;
use crate::ui::format::{format_axis_time, format_si};
use ratatui::prelude::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct PlotBounds {
    pub(super) left: u16,
    pub(super) right: u16,
    pub(super) top: u16,
    pub(super) bottom: u16,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct YLabelArea {
    pub(super) left: u16,
    pub(super) width: u16,
}

pub(super) fn y_label_width(
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

pub(super) fn render_intermediate_y_labels(
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

pub(super) fn render_autogrid_time_labels(
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
}
