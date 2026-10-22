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
    widgets::{Block, Borders, Gauge},
};

pub(super) fn render_gauge(frame: &mut Frame, area: Rect, p: &PanelState, app: &AppState) {
    let theme = &app.theme;

    // Preserve the first available value across visible series. If every
    // visible series is null, retain its name and display Grafana's noValue.
    let available_value = p
        .series
        .iter()
        .filter(|s| s.visible)
        .find_map(|s| s.value.map(|value| (value, s.name.clone())));
    let has_null_visible_value = available_value.is_none() && p.series.iter().any(|s| s.visible);
    let (value, name, value_label) = match available_value {
        Some((value, name)) => (value, name, p.display.format_value(Some(value))),
        None => match p.series.iter().find(|s| s.visible) {
            Some(series) => (0.0, series.name.clone(), p.display.format_value(None)),
            None => (
                0.0,
                "No data".to_string(),
                p.display.format_value(Some(0.0)),
            ),
        },
    };

    let min = p.min.unwrap_or(0.0);
    let max = p
        .max
        .unwrap_or(if value > 100.0 { value * 1.2 } else { 100.0 });

    let color = p.get_color_for_value(value).unwrap_or(theme.palette[0]);

    let ratio = if !has_null_visible_value && max > min {
        ((value - min) / (max - min)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
        .ratio(ratio)
        .label(format!("{value_label} ({name})"));

    frame.render_widget(gauge, area);
}
