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

use super::state::{AppMode, AppState, YAxisMode};
use crate::ui;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Rect, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InputAction {
    Redraw,
    Quit,
}

enum SharedKeyResult {
    Handled,
    Quit,
    Unhandled,
}

pub(super) async fn handle_key(key: KeyEvent, app: &mut AppState) -> Result<InputAction> {
    let action = match app.mode {
        AppMode::Search => handle_search_key(key, app),
        AppMode::Inspect => handle_inspect_key(key, app),
        AppMode::Fullscreen => handle_fullscreen_key(key, app).await?,
        AppMode::FullscreenInspect => handle_fullscreen_inspect_key(key, app),
        AppMode::Normal => handle_normal_key(key, app).await?,
    };
    Ok(action)
}

pub(super) fn handle_mouse(
    mouse: MouseEvent,
    terminal_size: Size,
    app: &mut AppState,
) -> Result<InputAction> {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) => {
            let rect = Rect::new(0, 0, terminal_size.width, terminal_size.height);
            if let Some((idx, panel_rect)) = ui::hit_test(app, rect, mouse.column, mouse.row) {
                app.selected_panel = idx;

                match app.mode {
                    AppMode::Normal | AppMode::Inspect => {}
                    AppMode::Fullscreen | AppMode::FullscreenInspect => {
                        app.mode = AppMode::FullscreenInspect;

                        let chart_width = panel_rect.width.saturating_sub(2) as f64;
                        if chart_width > 0.0 {
                            let relative_x = (mouse.column.saturating_sub(panel_rect.x + 1)) as f64;
                            let fraction = (relative_x / chart_width).clamp(0.0, 1.0);
                            let (start_ts, _) = app.time_bounds();
                            app.cursor_x = Some(start_ts + fraction * app.range.as_secs_f64());
                        }
                    }
                    _ => {}
                }
            }
            Ok(InputAction::Redraw)
        }
        MouseEventKind::ScrollDown => {
            app.vertical_scroll = app.vertical_scroll.saturating_add(1);
            Ok(InputAction::Redraw)
        }
        MouseEventKind::ScrollUp => {
            app.vertical_scroll = app.vertical_scroll.saturating_sub(1);
            Ok(InputAction::Redraw)
        }
        _ => Ok(InputAction::Redraw),
    }
}

fn handle_search_key(key: KeyEvent, app: &mut AppState) -> InputAction {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            app.search_query.clear();
            app.search_results.clear();
        }
        KeyCode::Enter => {
            if let Some(&idx) = app.search_results.first() {
                app.selected_panel = idx;
                app.mode = AppMode::Fullscreen;
                app.search_query.clear();
                app.search_results.clear();
            }
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            update_search_results(app);
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            update_search_results(app);
        }
        _ => {}
    }
    InputAction::Redraw
}

fn handle_inspect_key(key: KeyEvent, app: &mut AppState) -> InputAction {
    match key.code {
        KeyCode::Esc | KeyCode::Char('v') => {
            app.mode = AppMode::Normal;
            app.cursor_x = None;
            InputAction::Redraw
        }
        KeyCode::Left => {
            app.move_cursor(-1);
            InputAction::Redraw
        }
        KeyCode::Right => {
            app.move_cursor(1);
            InputAction::Redraw
        }
        KeyCode::Char('q') => InputAction::Quit,
        _ => InputAction::Redraw,
    }
}

async fn handle_fullscreen_key(key: KeyEvent, app: &mut AppState) -> Result<InputAction> {
    let action = match key.code {
        KeyCode::Esc | KeyCode::Char('f') | KeyCode::Enter => {
            app.mode = AppMode::Normal;
            InputAction::Redraw
        }
        KeyCode::Char('v') => {
            app.mode = AppMode::FullscreenInspect;
            app.center_cursor();
            InputAction::Redraw
        }
        KeyCode::PageUp => {
            app.select_previous_panel();
            InputAction::Redraw
        }
        KeyCode::PageDown => {
            app.select_next_panel();
            InputAction::Redraw
        }
        _ => shared_key_action(handle_shared_keys(key, app).await?),
    };
    Ok(action)
}

fn handle_fullscreen_inspect_key(key: KeyEvent, app: &mut AppState) -> InputAction {
    match key.code {
        KeyCode::Esc | KeyCode::Char('v') => {
            app.mode = AppMode::Fullscreen;
            app.cursor_x = None;
            InputAction::Redraw
        }
        KeyCode::Char('g') => {
            app.autogrid_enabled = !app.autogrid_enabled;
            InputAction::Redraw
        }
        KeyCode::Left => {
            app.move_cursor(-1);
            InputAction::Redraw
        }
        KeyCode::Right => {
            app.move_cursor(1);
            InputAction::Redraw
        }
        KeyCode::Char('q') => InputAction::Quit,
        _ => InputAction::Redraw,
    }
}

async fn handle_normal_key(key: KeyEvent, app: &mut AppState) -> Result<InputAction> {
    let action = match key.code {
        KeyCode::Char('f') => {
            app.mode = AppMode::Fullscreen;
            InputAction::Redraw
        }
        KeyCode::Char('v') => {
            app.mode = AppMode::Inspect;
            app.center_cursor();
            InputAction::Redraw
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_previous_panel();
            InputAction::Redraw
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next_panel();
            InputAction::Redraw
        }
        KeyCode::PageUp => {
            app.vertical_scroll = app.vertical_scroll.saturating_sub(10);
            InputAction::Redraw
        }
        KeyCode::PageDown => {
            app.vertical_scroll = app.vertical_scroll.saturating_add(10);
            InputAction::Redraw
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            toggle_series_visibility(app, c);
            InputAction::Redraw
        }
        KeyCode::Home => {
            app.vertical_scroll = 0;
            InputAction::Redraw
        }
        KeyCode::End => {
            app.vertical_scroll = usize::MAX;
            InputAction::Redraw
        }
        KeyCode::Char('?') => {
            app.debug_bar = !app.debug_bar;
            InputAction::Redraw
        }
        KeyCode::Char('/') => {
            app.mode = AppMode::Search;
            app.search_query.clear();
            app.search_results.clear();
            InputAction::Redraw
        }
        _ => shared_key_action(handle_shared_keys(key, app).await?),
    };
    Ok(action)
}

async fn handle_shared_keys(key: KeyEvent, app: &mut AppState) -> Result<SharedKeyResult> {
    match key.code {
        KeyCode::Char('q') => Ok(SharedKeyResult::Quit),
        KeyCode::Char('r') | KeyCode::Char('R') => {
            app.refresh().await?;
            Ok(SharedKeyResult::Handled)
        }
        KeyCode::Char('+') => {
            app.zoom_out();
            app.refresh().await?;
            Ok(SharedKeyResult::Handled)
        }
        KeyCode::Char('-') => {
            app.zoom_in();
            app.refresh().await?;
            Ok(SharedKeyResult::Handled)
        }
        KeyCode::Char('[') if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.pan_left();
            app.refresh().await?;
            Ok(SharedKeyResult::Handled)
        }
        KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.pan_left();
            app.refresh().await?;
            Ok(SharedKeyResult::Handled)
        }
        KeyCode::Char(']') if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.pan_right();
            app.refresh().await?;
            Ok(SharedKeyResult::Handled)
        }
        KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.pan_right();
            app.refresh().await?;
            Ok(SharedKeyResult::Handled)
        }
        KeyCode::Char('0') => {
            app.reset_to_live();
            app.refresh().await?;
            Ok(SharedKeyResult::Handled)
        }
        KeyCode::Char('y') => {
            if let Some(panel) = app.panels.get_mut(app.selected_panel) {
                panel.y_axis_mode = match panel.y_axis_mode {
                    YAxisMode::Auto => YAxisMode::ZeroBased,
                    YAxisMode::ZeroBased => YAxisMode::Auto,
                };
            }
            Ok(SharedKeyResult::Handled)
        }
        KeyCode::Char('g') => {
            app.autogrid_enabled = !app.autogrid_enabled;
            Ok(SharedKeyResult::Handled)
        }
        _ => Ok(SharedKeyResult::Unhandled),
    }
}

fn shared_key_action(result: SharedKeyResult) -> InputAction {
    match result {
        SharedKeyResult::Handled | SharedKeyResult::Unhandled => InputAction::Redraw,
        SharedKeyResult::Quit => InputAction::Quit,
    }
}

fn update_search_results(app: &mut AppState) {
    if app.search_query.is_empty() {
        app.search_results.clear();
        return;
    }

    let query = app.search_query.to_lowercase();
    app.search_results = app
        .panels
        .iter()
        .enumerate()
        .filter(|(_, panel)| panel.title.to_lowercase().contains(&query))
        .map(|(i, _)| i)
        .collect();
}

fn toggle_series_visibility(app: &mut AppState, c: char) {
    let Some(digit) = c.to_digit(10) else {
        return;
    };
    let Some(panel) = app.panels.get_mut(app.selected_panel) else {
        return;
    };

    if digit == 0 {
        for series in &mut panel.series {
            series.visible = true;
        }
    } else {
        let idx = (digit - 1) as usize;
        if let Some(series) = panel.series.get_mut(idx) {
            series.visible = !series.visible;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{PanelState, PanelType, SeriesView};
    use crate::export::ExportOptions;
    use crate::prom;
    use crate::theme::Theme;
    use std::time::Duration;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn test_app() -> AppState {
        AppState::new(
            prom::PromClient::new("http://localhost:9090".to_string()),
            Duration::from_secs(3600),
            Duration::from_secs(60),
            Duration::from_millis(1000),
            "Test".to_string(),
            vec![test_panel("CPU"), test_panel("Memory")],
            0,
            Theme::default(),
            "dashed".to_string(),
            ExportOptions::default(),
        )
    }

    fn test_panel(title: &str) -> PanelState {
        PanelState {
            title: title.to_string(),
            exprs: vec![],
            legends: vec![],
            series: vec![
                SeriesView {
                    name: "a".to_string(),
                    value: Some(1.0),
                    points: vec![],
                    visible: true,
                },
                SeriesView {
                    name: "b".to_string(),
                    value: Some(2.0),
                    points: vec![],
                    visible: false,
                },
            ],
            last_error: None,
            last_url: None,
            last_samples: 0,
            grid: None,
            y_axis_mode: YAxisMode::Auto,
            panel_type: PanelType::Graph,
            thresholds: None,
            min: None,
            max: None,
            autogrid: None,
        }
    }

    #[tokio::test]
    async fn normal_navigation_updates_selected_panel() {
        let mut app = test_app();

        handle_key(key(KeyCode::Char('j')), &mut app).await.unwrap();
        assert_eq!(app.selected_panel, 1);

        handle_key(key(KeyCode::Char('k')), &mut app).await.unwrap();
        assert_eq!(app.selected_panel, 0);
    }

    #[tokio::test]
    async fn normal_digit_keys_toggle_series_and_zero_shows_all() {
        let mut app = test_app();

        handle_key(key(KeyCode::Char('1')), &mut app).await.unwrap();
        assert!(!app.panels[0].series[0].visible);

        handle_key(key(KeyCode::Char('0')), &mut app).await.unwrap();
        assert!(app.panels[0].series.iter().all(|series| series.visible));
    }

    #[tokio::test]
    async fn search_keys_update_query_results_and_selection() {
        let mut app = test_app();

        handle_key(key(KeyCode::Char('/')), &mut app).await.unwrap();
        assert_eq!(app.mode, AppMode::Search);

        handle_key(key(KeyCode::Char('m')), &mut app).await.unwrap();
        assert_eq!(app.search_query, "m");
        assert_eq!(app.search_results, vec![1]);

        handle_key(key(KeyCode::Backspace), &mut app).await.unwrap();
        assert!(app.search_query.is_empty());
        assert!(app.search_results.is_empty());

        handle_key(key(KeyCode::Char('c')), &mut app).await.unwrap();
        handle_key(key(KeyCode::Enter), &mut app).await.unwrap();
        assert_eq!(app.selected_panel, 0);
        assert_eq!(app.mode, AppMode::Fullscreen);

        handle_key(key(KeyCode::Esc), &mut app).await.unwrap();
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[tokio::test]
    async fn fullscreen_keys_update_mode_and_selection() {
        let mut app = test_app();
        app.mode = AppMode::Fullscreen;

        handle_key(key(KeyCode::PageDown), &mut app).await.unwrap();
        assert_eq!(app.selected_panel, 1);

        handle_key(key(KeyCode::PageUp), &mut app).await.unwrap();
        assert_eq!(app.selected_panel, 0);

        handle_key(key(KeyCode::Char('v')), &mut app).await.unwrap();
        assert_eq!(app.mode, AppMode::FullscreenInspect);
        assert!(app.cursor_x.is_some());

        handle_key(key(KeyCode::Esc), &mut app).await.unwrap();
        assert_eq!(app.mode, AppMode::Fullscreen);
        assert!(app.cursor_x.is_none());
    }

    #[tokio::test]
    async fn shared_keys_toggle_autogrid_and_y_axis_mode() {
        let mut app = test_app();

        handle_key(key(KeyCode::Char('g')), &mut app).await.unwrap();
        assert!(!app.autogrid_enabled);

        handle_key(key(KeyCode::Char('y')), &mut app).await.unwrap();
        assert_eq!(app.panels[0].y_axis_mode, YAxisMode::ZeroBased);
    }
}
