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
use crate::ui::format::format_si;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Sparkline},
};

pub(super) fn render_stat(frame: &mut Frame, area: Rect, p: &PanelState, app: &AppState) {
    let theme = &app.theme;

    // Find the latest value from the first visible series
    let (value, name) = p
        .series
        .iter()
        .filter(|s| s.visible)
        .find_map(|s| s.value.map(|v| (v, s.name.clone())))
        .unwrap_or((0.0, "No data".to_string()));

    let color = p.get_color_for_value(value).unwrap_or(theme.palette[0]);

    // Split area into value (top) and sparkline (bottom)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Render Big Value
    let val_str = format_si(value);
    let big_value = Paragraph::new(val_str)
        .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::NONE));

    frame.render_widget(big_value, chunks[0]);

    // Render Sparkline
    if let Some(s) = p.series.iter().find(|s| s.visible && s.name == name) {
        let data: Vec<u64> = s.points.iter().map(|(_, v)| *v as u64).collect();
        let sparkline = Sparkline::default()
            .block(Block::default().borders(Borders::NONE))
            .data(&data)
            .style(Style::default().fg(color));
        frame.render_widget(sparkline, chunks[1]);
    }
}
