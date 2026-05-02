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

mod bar_gauge;
mod gauge;
mod graph;
mod heatmap;
mod stat;
mod table;

use crate::app::{AppState, PanelState, PanelType};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use bar_gauge::render_bar_gauge;
use gauge::render_gauge;
pub(crate) use graph::calculate_y_bounds;
use graph::render_graph_panel;
use heatmap::render_heatmap;
use stat::render_stat;
use table::render_table;

/// Renders a single panel.
///
/// This function handles:
/// - Drawing the panel border and title.
/// - Rendering the chart with data series.
/// - Drawing the legend (if space permits).
/// - Handling inspection mode (cursor line and values).
/// - Displaying error messages if the panel has an error.
pub(crate) fn render_panel(
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

    match p.panel_type {
        PanelType::Graph | PanelType::Unknown => {
            render_graph_panel(frame, inner_area, p, app, cursor_x);
        }
        PanelType::Gauge => {
            render_gauge(frame, inner_area, p, app);
        }
        PanelType::BarGauge => {
            render_bar_gauge(frame, inner_area, p, app);
        }
        PanelType::Table => {
            render_table(frame, inner_area, p, app);
        }
        PanelType::Stat => {
            render_stat(frame, inner_area, p, app);
        }
        PanelType::Heatmap => {
            render_heatmap(frame, inner_area, p, app);
        }
    }
}
