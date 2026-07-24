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

use super::labels::{PlotBounds, value_to_plot_x};
use super::overlay::overlay_cell_if_blank_or_weak_area_fill;
use crate::annotations::{AnnotationCluster, AnnotationEvent, cluster_events_by};
use ratatui::prelude::*;

pub(super) fn cluster_badge(count: usize) -> char {
    match count {
        0 => ' ',
        1 => '•',
        2..=9 => char::from_digit(count as u32, 10).unwrap(),
        _ => '+',
    }
}

pub(super) fn terminal_clusters<'a>(
    events: Vec<&'a AnnotationEvent>,
    x_bounds: [f64; 2],
    plot: PlotBounds,
) -> Vec<AnnotationCluster<'a>> {
    cluster_events_by(events, |timestamp| {
        value_to_plot_x(timestamp, x_bounds, plot).map(u32::from)
    })
}

pub(super) fn render_annotation_clusters(
    frame: &mut Frame,
    clusters: &[AnnotationCluster<'_>],
    plot: PlotBounds,
    strong_data: &ratatui::buffer::Buffer,
    color: Color,
) {
    for cluster in clusters {
        let Ok(x) = u16::try_from(cluster.coordinate) else {
            continue;
        };
        if x < plot.left || x >= plot.right {
            continue;
        }

        render_marker_cell(
            frame,
            strong_data,
            x,
            plot.top,
            cluster_badge(cluster.events.len()),
            color,
        );
        for y in plot.top.saturating_add(1)..=plot.bottom {
            render_marker_cell(frame, strong_data, x, y, '┊', color);
        }
    }
}

fn render_marker_cell(
    frame: &mut Frame,
    strong_data: &ratatui::buffer::Buffer,
    x: u16,
    y: u16,
    marker: char,
    color: Color,
) {
    let Some(destination) = frame.buffer_mut().cell_mut((x, y)) else {
        return;
    };
    let Some(strong) = strong_data.cell((x, y)) else {
        return;
    };
    overlay_marker_cell(destination, strong, marker, color);
}

fn overlay_marker_cell(
    destination: &mut ratatui::buffer::Cell,
    strong_data: &ratatui::buffer::Cell,
    marker: char,
    color: Color,
) {
    let mut source = ratatui::buffer::Cell::default();
    source
        .set_char(marker)
        .set_style(Style::default().fg(color));
    overlay_cell_if_blank_or_weak_area_fill(destination, &source, strong_data);
}

pub(super) fn active_cluster<'a>(
    clusters: &'a [AnnotationCluster<'a>],
    cursor_x: Option<f64>,
    x_bounds: [f64; 2],
    plot: PlotBounds,
) -> Option<&'a AnnotationCluster<'a>> {
    let cursor_column = value_to_plot_x(cursor_x?, x_bounds, plot)?;
    clusters
        .iter()
        .find(|cluster| cluster.coordinate == u32::from(cursor_column))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn badge_matches_cluster_size() {
        assert_eq!(cluster_badge(1), '•');
        assert_eq!(cluster_badge(2), '2');
        assert_eq!(cluster_badge(9), '9');
        assert_eq!(cluster_badge(10), '+');
    }

    #[test]
    fn marker_does_not_replace_strong_data_cell() {
        let mut destination = ratatui::buffer::Cell::default();
        destination.set_char('x');
        let strong = ratatui::buffer::Cell::default();

        overlay_marker_cell(&mut destination, &strong, '┊', Color::Yellow);

        assert_eq!(destination.symbol(), "x");
    }
}
