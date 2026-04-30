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
    widgets::{Block, Borders, Gauge},
};

pub(super) fn render_gauge(frame: &mut Frame, area: Rect, p: &PanelState, app: &AppState) {
    let theme = &app.theme;

    // Find the latest value from the first visible series
    let (value, name) = p
        .series
        .iter()
        .filter(|s| s.visible)
        .find_map(|s| s.value.map(|v| (v, s.name.clone())))
        .unwrap_or((0.0, "No data".to_string()));

    let min = p.min.unwrap_or(0.0);
    let max = p
        .max
        .unwrap_or(if value > 100.0 { value * 1.2 } else { 100.0 });

    let color = p.get_color_for_value(value).unwrap_or(theme.palette[0]);

    let ratio = if max > min {
        ((value - min) / (max - min)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
        .ratio(ratio)
        .label(format!("{} ({})", format_si(value), name));

    frame.render_widget(gauge, area);
}
