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
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Paragraph},
};

pub(super) fn render_bar_gauge(frame: &mut Frame, area: Rect, p: &PanelState, app: &AppState) {
    let theme = &app.theme;

    let mut max_label_len = 3;

    let scale = 1000.0;

    // Map intermediate valid series
    let mut valid_series: Vec<_> = p
        .series
        .iter()
        .filter(|s| s.visible && s.value.is_some())
        .collect();

    // Sort descending safely
    valid_series.sort_by(|a, b| {
        let v_a = a.value.unwrap_or(0.0);
        let v_b = b.value.unwrap_or(0.0);
        v_b.partial_cmp(&v_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Truncate based on area width
    let max_bars = (area.width / 4).saturating_sub(1).max(1) as usize;
    valid_series.truncate(max_bars);

    let mut bars = Vec::with_capacity(valid_series.len());

    for s in valid_series {
        let v = s.value.unwrap();
        max_label_len = max_label_len.max(s.name.len());
        let color = p.get_color_for_value(v).unwrap_or(theme.palette[0]);
        let bar = Bar::default()
            .value((v * scale) as u64)
            .text_value(format_si(v))
            .label(Line::from(s.name.as_str()))
            .style(Style::default().fg(color))
            .value_style(Style::default().fg(theme.text).bg(color));
        bars.push(bar);
    }

    if bars.is_empty() {
        let para = Paragraph::new("No data").style(Style::default().fg(theme.text));
        frame.render_widget(para, area);
        return;
    }

    let bar_width = (area.width / bars.len() as u16)
        .saturating_sub(1)
        .min(max_label_len as u16)
        .max(3);

    let bar_group = BarGroup::default().bars(&bars);

    let bar_chart = BarChart::default()
        .block(Block::default().borders(Borders::NONE))
        .data(bar_group)
        .bar_width(bar_width)
        .bar_gap(1);

    frame.render_widget(bar_chart, area);
}
