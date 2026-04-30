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
    widgets::{Block, Borders, Paragraph, Row, Table},
};

pub(super) fn render_table(frame: &mut Frame, area: Rect, p: &PanelState, app: &AppState) {
    let theme = &app.theme;

    let header = ["Series", "Value"];
    let rows: Vec<Row> = p
        .series
        .iter()
        .filter(|s| s.visible)
        .map(|s| {
            let val_str = s.value.map(format_si).unwrap_or_else(|| "-".to_string());
            let color = s
                .value
                .and_then(|v| p.get_color_for_value(v))
                .unwrap_or(theme.text);

            Row::new(vec![
                Span::styled(s.name.clone(), Style::default().fg(theme.text)),
                Span::styled(val_str, Style::default().fg(color)),
            ])
        })
        .collect();

    if rows.is_empty() {
        let para = Paragraph::new("No data").style(Style::default().fg(theme.text));
        frame.render_widget(para, area);
        return;
    }

    let table = Table::new(
        rows,
        [Constraint::Percentage(70), Constraint::Percentage(30)],
    )
    .header(
        Row::new(header)
            .style(
                Style::default()
                    .fg(theme.title)
                    .add_modifier(Modifier::BOLD),
            )
            .bottom_margin(1),
    )
    .block(Block::default().borders(Borders::NONE))
    .column_spacing(1);

    frame.render_widget(table, area);
}
