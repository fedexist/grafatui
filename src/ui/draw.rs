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

use super::layout::{calculate_grid_layout, calculate_two_column_layout};
use super::panels::render_panel;
use crate::app::{AppMode, AppState, PanelState};
use humantime::format_duration;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

pub(crate) fn draw_ui(frame: &mut Frame, app: &AppState) {
    let size = frame.area();

    // Layout: title bar, charts area, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(size);

    // Title
    let title_text = format!(
        "{} — range={} step={}  panels={}  {}(r to refresh, +/- range, [] pan, 0 live, q quit)",
        app.title,
        format_duration(app.range),
        format_duration(app.step),
        app.panels.len(),
        if app.is_live() { "" } else { "⏸ PAUSED " }
    );
    let title_block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(title_text).alignment(Alignment::Center));
    frame.render_widget(title_block, chunks[0]);

    // Charts area: use Grafana grid if any panel has it, else fallback to 2-column flow
    let area = chunks[1];
    let charts_block = Block::default().borders(Borders::ALL);
    frame.render_widget(charts_block, area);
    let inner_area = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    if app.mode == AppMode::Fullscreen || app.mode == AppMode::FullscreenInspect {
        if let Some(p) = app.panels.get(app.selected_panel) {
            render_panel(frame, inner_area, p, app, true, app.cursor_x);
        }
    } else {
        let has_grid = app.panels.iter().any(|p| p.grid.is_some());

        let panel_rects = if has_grid {
            calculate_grid_layout(inner_area, app)
        } else {
            calculate_two_column_layout(inner_area, app)
        };

        for (rect, panel_idx) in &panel_rects {
            // eprintln!("Rendering panel {} at {:?}", panel_idx, rect);
            if let Some(p) = app.panels.get(*panel_idx) {
                let is_selected = *panel_idx == app.selected_panel;
                render_panel(frame, *rect, p, app, is_selected, app.cursor_x);
            }
        }

        if !has_grid && app.panels.is_empty() {
            // No panels to render
        } else if has_grid {
            // Check if we need to render extras (panels without grid)
            // The calculate_grid_layout should handle extras too?
            // The original code handled extras by stacking them below.
            // Let's make calculate_grid_layout return extras too.
        }
    }

    // Footer / Status bar
    let errors = app.panels.iter().filter(|p| p.last_error.is_some()).count();
    let panel_count_display =
        if app.mode == AppMode::Fullscreen || app.mode == AppMode::FullscreenInspect {
            "1 (Fullscreen)".to_string()
        } else {
            format!("{}", app.panels.len())
        };

    let mode_display = match app.mode {
        AppMode::Normal => "NORMAL",
        AppMode::Search => "SEARCH",
        AppMode::Fullscreen => "FULLSCREEN",
        AppMode::Inspect => "INSPECT",
        AppMode::FullscreenInspect => "FULLSCREEN INSPECT",
    };

    let summary = format!(
        "Mode: {}{} | Prom: {} | range={} step={:?} refresh={} | grid={} | panels={} (skipped {}) errors={} | keys: ↑/↓ scroll, r refresh, e export, Ctrl+E record, +/- range, q quit, ? debug:{}",
        mode_display,
        if app.recording.is_some() { " REC" } else { "" },
        app.prometheus.base,
        format_duration(app.range),
        app.step,
        format_duration(app.refresh_every),
        if app.autogrid_enabled { "on" } else { "off" },
        panel_count_display,
        app.skipped_panels,
        errors,
        if app.debug_bar { "on" } else { "off" }
    );

    let mut detail = String::new();
    if let Some(status) = &app.export_status {
        detail = status.clone();
    }
    if app.debug_bar {
        // Choose a debug panel: if we have grid, pick the top-left grid panel; otherwise pick the first panel
        let debug_panel: Option<&PanelState> = if app.panels.iter().any(|p| p.grid.is_some()) {
            app.panels
                .iter()
                .filter(|p| p.grid.is_some())
                .min_by_key(|p| {
                    let g = p.grid.unwrap();
                    (g.y, g.x)
                })
        } else {
            app.panels.first()
        };

        if let Some(p) = debug_panel {
            let url = p.last_url.as_deref().unwrap_or("-");
            detail = format!(
                "last panel: {} | samples={} | url={} ",
                p.title, p.last_samples, url
            );
        }
    }

    if app.mode == AppMode::Inspect {
        if let Some(cx) = app.cursor_x {
            let cursor_time = chrono::DateTime::from_timestamp(cx as i64, 0)
                .map(|dt| dt.format("%H:%M:%S").to_string())
                .unwrap_or_default();
            detail = format!("Cursor: {} | {}", cursor_time, detail);
        }
    }

    let footer = Paragraph::new(format!("{}\n{}", summary, detail)).wrap(Wrap { trim: true });
    frame.render_widget(footer, chunks[2]);

    // Search Popup
    if app.mode == AppMode::Search {
        let area = centered_rect(60, 20, size);
        let block = Block::default()
            .title(" Search Panels ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border_selected));
        frame.render_widget(Clear, area); // Clear background
        frame.render_widget(block, area);

        let inner_area = area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        });
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner_area);

        // Input
        let input = Paragraph::new(format!("> {}", app.search_query))
            .style(Style::default().fg(app.theme.text));
        frame.render_widget(input, chunks[0]);

        // Results
        let results: Vec<ListItem> = app
            .search_results
            .iter()
            .map(|&idx| {
                let p = &app.panels[idx];
                ListItem::new(format!("• {}", p.title))
            })
            .collect();
        let list = List::new(results)
            .block(Block::default().borders(Borders::TOP))
            .highlight_style(
                Style::default()
                    .fg(app.theme.title)
                    .add_modifier(Modifier::BOLD)
                    .bg(app.theme.background), // Optional: add background to make it pop more?
            )
            .highlight_symbol(">> ");

        let mut list_state = ratatui::widgets::ListState::default();
        if !app.search_results.is_empty() {
            list_state.select(Some(0));
        }
        frame.render_stateful_widget(list, chunks[1], &mut list_state);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
