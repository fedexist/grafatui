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

use crate::app::{AppState, PanelState};
use crate::ui::format::value_to_heatmap_color;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

pub(super) fn render_heatmap(frame: &mut Frame, area: Rect, p: &PanelState, app: &AppState) {
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
