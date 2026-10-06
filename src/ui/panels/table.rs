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

use crate::app::{AppState, PanelState};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Row, Table, TableState},
};

pub(super) fn render_table(
    frame: &mut Frame,
    area: Rect,
    p: &PanelState,
    app: &AppState,
    context: super::PanelRenderContext,
) {
    let theme = &app.theme;

    let header = ["Series", "Value"];
    let rows: Vec<Row> = super::prepare_table_rows(p)
        .into_iter()
        .map(|row| {
            Row::new(vec![
                Span::styled(row.name, Style::default().fg(theme.text)),
                Span::styled(
                    row.value,
                    Style::default().fg(row.color.unwrap_or(theme.text)),
                ),
            ])
        })
        .collect();

    if rows.is_empty() {
        let para = Paragraph::new("No data").style(Style::default().fg(theme.text));
        frame.render_widget(para, area);
        return;
    }

    let rows_len = rows.len();
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

    if context.content_fit {
        let offset = super::content::panel_body_offset(
            rows_len as u64,
            context.body_offset,
            context.local_body_offset,
            area.height.saturating_sub(2),
        ) as usize;
        frame.render_stateful_widget(table, area, &mut TableState::default().with_offset(offset));
    } else {
        frame.render_widget(table, area);
    }
}
