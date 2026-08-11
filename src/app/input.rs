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

use super::state::{AppMode, AppState, YAxisMode};
use crate::annotations::AnnotationModal;
use crate::ui;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Rect, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InputAction {
    Redraw,
    Quit,
    ExportCurrent,
    ToggleRecording,
}

enum SharedKeyResult {
    Handled,
    Quit,
    Unhandled,
}

pub(super) async fn handle_key(
    key: KeyEvent,
    terminal_size: Size,
    app: &mut AppState,
) -> Result<InputAction> {
    if key.code == KeyCode::Char('e') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Ok(InputAction::ToggleRecording);
    }

    if key.code == KeyCode::Char('e') && key.modifiers.is_empty() && app.mode != AppMode::Search {
        return Ok(InputAction::ExportCurrent);
    }

    if app.annotation_modal.is_some() {
        return Ok(handle_annotation_modal_key(key, terminal_size, app));
    }

    if key.code == KeyCode::Char('t') && key.modifiers.is_empty() && app.mode != AppMode::Search {
        app.open_tag_filter_modal();
        return Ok(InputAction::Redraw);
    }

    if key.code == KeyCode::Char('a') && key.modifiers.is_empty() && app.mode != AppMode::Search {
        app.annotations.toggle_visibility();
        return Ok(InputAction::Redraw);
    }

    let action = match app.mode {
        AppMode::Search => handle_search_key(key, app),
        AppMode::Inspect => handle_inspect_key(key, app),
        AppMode::Fullscreen => handle_fullscreen_key(key, app).await?,
        AppMode::FullscreenInspect => handle_fullscreen_inspect_key(key, app),
        AppMode::Normal => handle_normal_key(key, app).await?,
    };
    Ok(action)
}

fn handle_annotation_modal_key(
    key: KeyEvent,
    terminal_size: Size,
    app: &mut AppState,
) -> InputAction {
    match key.code {
        KeyCode::Esc => app.annotation_modal = None,
        KeyCode::Enter => {
            let next_filter = match app.annotation_modal.as_ref() {
                Some(AnnotationModal::TagFilter(state)) => Some(state.draft().clone()),
                Some(AnnotationModal::Cluster(_)) | None => None,
            };
            app.annotation_modal = None;
            if let Some(filter) = next_filter {
                app.annotations.set_filter(filter);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => match app.annotation_modal.as_mut() {
            Some(AnnotationModal::Cluster(state)) => state.move_by(-1),
            Some(AnnotationModal::TagFilter(state)) => state.move_by(-1),
            None => {}
        },
        KeyCode::Down | KeyCode::Char('j') => match app.annotation_modal.as_mut() {
            Some(AnnotationModal::Cluster(state)) => state.move_by(1),
            Some(AnnotationModal::TagFilter(state)) => state.move_by(1),
            None => {}
        },
        KeyCode::PageUp | KeyCode::PageDown => {
            let direction = if key.code == KeyCode::PageUp { -1 } else { 1 };
            let rows = ui::annotation_cluster_page_size(terminal_size);
            if let Some(AnnotationModal::Cluster(state)) = app.annotation_modal.as_mut() {
                state.move_page(direction, rows);
            }
        }
        KeyCode::Char(' ') => {
            if let Some(AnnotationModal::TagFilter(state)) = app.annotation_modal.as_mut() {
                state.toggle_selected();
            }
        }
        KeyCode::Char('c') => {
            if let Some(AnnotationModal::TagFilter(state)) = app.annotation_modal.as_mut() {
                state.clear();
            }
        }
        _ => {}
    }
    InputAction::Redraw
}

pub(super) fn handle_mouse(
    mouse: MouseEvent,
    terminal_size: Size,
    app: &mut AppState,
) -> Result<InputAction> {
    if app.annotation_modal.is_some() {
        return Ok(InputAction::Redraw);
    }

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
        KeyCode::Enter => {
            app.open_rendered_annotation_cluster();
            InputAction::Redraw
        }
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
        KeyCode::Enter => {
            app.open_rendered_annotation_cluster();
            InputAction::Redraw
        }
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
    use crate::app::{GraphOptions, PanelOptions, PanelState, PanelType, SeriesView};
    use crate::export::ExportOptions;
    use crate::prom;
    use crate::theme::Theme;
    use std::time::Duration;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    fn size() -> Size {
        Size::new(100, 40)
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
            query_modes: vec![],
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
            display: crate::ui::DisplayFormat::default(),
            options: PanelOptions::Graph(GraphOptions::default()),
        }
    }

    fn tagged_event(text: &str, tag: &str) -> crate::annotations::AnnotationEvent {
        let mut event = crate::annotations::test_event_at(50.0, text);
        event.tags = vec![tag.to_string()];
        event
    }

    #[tokio::test]
    async fn t_opens_tag_filter_except_in_search_or_when_disabled() {
        let mut app = test_app();
        app.annotations =
            crate::annotations::AnnotationState::from_events_for_test(vec![tagged_event(
                "release", "deploy",
            )]);

        handle_key(key(KeyCode::Char('t')), size(), &mut app)
            .await
            .unwrap();
        assert!(matches!(
            app.annotation_modal,
            Some(crate::annotations::AnnotationModal::TagFilter(_))
        ));

        app.annotation_modal = None;
        app.mode = AppMode::Search;
        handle_key(key(KeyCode::Char('t')), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(app.search_query, "t");

        let mut disabled = test_app();
        handle_key(key(KeyCode::Char('t')), size(), &mut disabled)
            .await
            .unwrap();
        assert!(disabled.annotation_modal.is_none());
    }

    #[tokio::test]
    async fn t_opens_tag_filter_while_annotation_markers_are_hidden() {
        let mut app = test_app();
        app.annotations =
            crate::annotations::AnnotationState::from_events_for_test(vec![tagged_event(
                "release", "deploy",
            )]);
        app.annotations.toggle_visibility();

        handle_key(key(KeyCode::Char('t')), size(), &mut app)
            .await
            .unwrap();

        assert!(matches!(
            app.annotation_modal,
            Some(crate::annotations::AnnotationModal::TagFilter(_))
        ));
    }

    #[tokio::test]
    async fn enter_opens_only_the_rendered_selected_cluster_in_inspect_modes() {
        let mut app = test_app();
        app.rendered_annotation_cluster =
            Some(vec![crate::annotations::test_event_at(50.0, "deploy")]);

        handle_key(key(KeyCode::Enter), size(), &mut app)
            .await
            .unwrap();
        assert!(app.annotation_modal.is_none());

        app.mode = AppMode::Inspect;
        handle_key(key(KeyCode::Enter), size(), &mut app)
            .await
            .unwrap();
        assert!(matches!(
            app.annotation_modal,
            Some(crate::annotations::AnnotationModal::Cluster(_))
        ));

        app.annotation_modal = None;
        app.mode = AppMode::FullscreenInspect;
        handle_key(key(KeyCode::Enter), size(), &mut app)
            .await
            .unwrap();
        assert!(matches!(
            app.annotation_modal,
            Some(crate::annotations::AnnotationModal::Cluster(_))
        ));
    }

    #[tokio::test]
    async fn modal_navigation_consumes_dashboard_navigation_and_zoom_keys() {
        let mut app = test_app();
        app.mode = AppMode::Inspect;
        app.cursor_x = Some(50.0);
        app.rendered_annotation_cluster = Some(vec![
            crate::annotations::test_event_at(50.0, "one"),
            crate::annotations::test_event_at(50.0, "two"),
            crate::annotations::test_event_at(50.0, "three"),
        ]);
        let original_range = app.range;
        let original_cursor = app.cursor_x;

        handle_key(key(KeyCode::Enter), size(), &mut app)
            .await
            .unwrap();
        handle_key(key(KeyCode::Char('j')), size(), &mut app)
            .await
            .unwrap();
        handle_key(key(KeyCode::PageDown), size(), &mut app)
            .await
            .unwrap();
        handle_key(key(KeyCode::Left), size(), &mut app)
            .await
            .unwrap();
        handle_key(key(KeyCode::Char('+')), size(), &mut app)
            .await
            .unwrap();

        let Some(crate::annotations::AnnotationModal::Cluster(modal)) =
            app.annotation_modal.as_ref()
        else {
            panic!("cluster modal should remain open");
        };
        assert_eq!(modal.selected_event().unwrap().text, "three");
        assert_eq!(app.range, original_range);
        assert_eq!(app.cursor_x, original_cursor);

        app.annotation_modal = None;
        app.mode = AppMode::Normal;
        app.annotations = crate::annotations::AnnotationState::from_events_for_test(vec![
            tagged_event("release", "deploy"),
            tagged_event("alert", "incident"),
        ]);
        handle_key(key(KeyCode::Char('t')), size(), &mut app)
            .await
            .unwrap();
        handle_key(key(KeyCode::Char('j')), size(), &mut app)
            .await
            .unwrap();
        handle_key(key(KeyCode::PageDown), size(), &mut app)
            .await
            .unwrap();
        handle_key(key(KeyCode::Char('+')), size(), &mut app)
            .await
            .unwrap();

        let Some(crate::annotations::AnnotationModal::TagFilter(modal)) =
            app.annotation_modal.as_ref()
        else {
            panic!("tag modal should remain open");
        };
        assert_eq!(modal.selected(), 1);
        assert_eq!(app.selected_panel, 0);
        assert_eq!(app.range, original_range);
    }

    #[tokio::test]
    async fn tag_filter_apply_cancel_and_clear_are_draft_isolated() {
        let mut app = test_app();
        app.annotations = crate::annotations::AnnotationState::from_events_for_test(vec![
            tagged_event("release", "deploy"),
            tagged_event("alert", "incident"),
        ]);

        handle_key(key(KeyCode::Char('t')), size(), &mut app)
            .await
            .unwrap();
        handle_key(key(KeyCode::Char(' ')), size(), &mut app)
            .await
            .unwrap();
        assert!(app.annotations.applied_filter().unwrap().is_empty());
        handle_key(key(KeyCode::Enter), size(), &mut app)
            .await
            .unwrap();
        assert!(app.annotation_modal.is_none());
        assert_eq!(
            app.annotations.applied_filter().unwrap().summary(),
            "deploy"
        );

        handle_key(key(KeyCode::Char('t')), size(), &mut app)
            .await
            .unwrap();
        handle_key(key(KeyCode::Char('j')), size(), &mut app)
            .await
            .unwrap();
        handle_key(key(KeyCode::Char(' ')), size(), &mut app)
            .await
            .unwrap();
        handle_key(key(KeyCode::Esc), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(
            app.annotations.applied_filter().unwrap().summary(),
            "deploy"
        );

        handle_key(key(KeyCode::Char('t')), size(), &mut app)
            .await
            .unwrap();
        handle_key(key(KeyCode::Char('c')), size(), &mut app)
            .await
            .unwrap();
        let Some(crate::annotations::AnnotationModal::TagFilter(modal)) =
            app.annotation_modal.as_ref()
        else {
            panic!("tag modal should remain open");
        };
        assert!(modal.draft().is_empty());
        assert_eq!(
            app.annotations.applied_filter().unwrap().summary(),
            "deploy"
        );
    }

    #[tokio::test]
    async fn enter_and_escape_close_cluster_without_changing_cursor_or_mode() {
        let mut app = test_app();
        app.mode = AppMode::Inspect;
        app.cursor_x = Some(50.0);
        app.rendered_annotation_cluster =
            Some(vec![crate::annotations::test_event_at(50.0, "deploy")]);

        handle_key(key(KeyCode::Enter), size(), &mut app)
            .await
            .unwrap();
        handle_key(key(KeyCode::Enter), size(), &mut app)
            .await
            .unwrap();
        assert!(app.annotation_modal.is_none());
        assert_eq!(app.mode, AppMode::Inspect);
        assert_eq!(app.cursor_x, Some(50.0));

        app.open_rendered_annotation_cluster();
        handle_key(key(KeyCode::Esc), size(), &mut app)
            .await
            .unwrap();
        assert!(app.annotation_modal.is_none());
        assert_eq!(app.mode, AppMode::Inspect);
        assert_eq!(app.cursor_x, Some(50.0));
    }

    #[tokio::test]
    async fn export_shortcuts_take_precedence_while_modal_is_open() {
        let mut app = test_app();
        app.annotations =
            crate::annotations::AnnotationState::from_events_for_test(vec![tagged_event(
                "release", "deploy",
            )]);
        app.open_tag_filter_modal();

        let action = handle_key(key(KeyCode::Char('e')), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(action, InputAction::ExportCurrent);
        assert!(app.annotation_modal.is_some());

        let action = handle_key(ctrl_key(KeyCode::Char('e')), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(action, InputAction::ToggleRecording);
        assert!(app.annotation_modal.is_some());
    }

    #[test]
    fn mouse_events_do_not_mutate_dashboard_state_while_modal_is_open() {
        let mut app = test_app();
        app.annotations =
            crate::annotations::AnnotationState::from_events_for_test(vec![tagged_event(
                "release", "deploy",
            )]);
        app.open_tag_filter_modal();
        app.mode = AppMode::FullscreenInspect;
        app.selected_panel = 1;
        app.cursor_x = Some(50.0);
        app.vertical_scroll = 3;

        for kind in [
            MouseEventKind::ScrollDown,
            MouseEventKind::Down(MouseButton::Left),
            MouseEventKind::Drag(MouseButton::Left),
        ] {
            let action = handle_mouse(
                MouseEvent {
                    kind,
                    column: 10,
                    row: 10,
                    modifiers: KeyModifiers::NONE,
                },
                size(),
                &mut app,
            )
            .unwrap();
            assert_eq!(action, InputAction::Redraw);
            assert_eq!(app.selected_panel, 1);
            assert_eq!(app.cursor_x, Some(50.0));
            assert_eq!(app.vertical_scroll, 3);
        }
    }

    #[tokio::test]
    async fn normal_navigation_updates_selected_panel() {
        let mut app = test_app();

        handle_key(key(KeyCode::Char('j')), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(app.selected_panel, 1);

        handle_key(key(KeyCode::Char('k')), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(app.selected_panel, 0);
    }

    #[tokio::test]
    async fn export_shortcuts_return_export_actions() {
        let mut app = test_app();

        let action = handle_key(key(KeyCode::Char('e')), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(action, InputAction::ExportCurrent);

        let action = handle_key(ctrl_key(KeyCode::Char('e')), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(action, InputAction::ToggleRecording);
    }

    #[tokio::test]
    async fn search_mode_e_keeps_typing_but_ctrl_e_toggles_recording() {
        let mut app = test_app();
        app.mode = AppMode::Search;

        let action = handle_key(key(KeyCode::Char('e')), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(action, InputAction::Redraw);
        assert_eq!(app.search_query, "e");

        let action = handle_key(ctrl_key(KeyCode::Char('e')), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(action, InputAction::ToggleRecording);
        assert_eq!(app.search_query, "e");
    }

    #[tokio::test]
    async fn annotations_toggle_in_normal_and_inspect_modes_but_types_in_search() {
        let mut app = test_app();
        app.annotations = crate::annotations::AnnotationState::from_events_for_test(vec![]);

        handle_key(key(KeyCode::Char('a')), size(), &mut app)
            .await
            .unwrap();
        assert!(!app.annotations.is_visible());

        app.mode = AppMode::Inspect;
        handle_key(key(KeyCode::Char('a')), size(), &mut app)
            .await
            .unwrap();
        assert!(app.annotations.is_visible());

        app.mode = AppMode::Search;
        handle_key(key(KeyCode::Char('a')), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(app.search_query, "a");
    }

    #[tokio::test]
    async fn normal_digit_keys_toggle_series_and_zero_shows_all() {
        let mut app = test_app();

        handle_key(key(KeyCode::Char('1')), size(), &mut app)
            .await
            .unwrap();
        assert!(!app.panels[0].series[0].visible);

        handle_key(key(KeyCode::Char('0')), size(), &mut app)
            .await
            .unwrap();
        assert!(app.panels[0].series.iter().all(|series| series.visible));
    }

    #[tokio::test]
    async fn search_keys_update_query_results_and_selection() {
        let mut app = test_app();

        handle_key(key(KeyCode::Char('/')), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(app.mode, AppMode::Search);

        handle_key(key(KeyCode::Char('m')), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(app.search_query, "m");
        assert_eq!(app.search_results, vec![1]);

        handle_key(key(KeyCode::Backspace), size(), &mut app)
            .await
            .unwrap();
        assert!(app.search_query.is_empty());
        assert!(app.search_results.is_empty());

        handle_key(key(KeyCode::Char('c')), size(), &mut app)
            .await
            .unwrap();
        handle_key(key(KeyCode::Enter), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(app.selected_panel, 0);
        assert_eq!(app.mode, AppMode::Fullscreen);

        handle_key(key(KeyCode::Esc), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[tokio::test]
    async fn fullscreen_keys_update_mode_and_selection() {
        let mut app = test_app();
        app.mode = AppMode::Fullscreen;

        handle_key(key(KeyCode::PageDown), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(app.selected_panel, 1);

        handle_key(key(KeyCode::PageUp), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(app.selected_panel, 0);

        handle_key(key(KeyCode::Char('v')), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(app.mode, AppMode::FullscreenInspect);
        assert!(app.cursor_x.is_some());

        handle_key(key(KeyCode::Esc), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(app.mode, AppMode::Fullscreen);
        assert!(app.cursor_x.is_none());
    }

    #[tokio::test]
    async fn shared_keys_toggle_autogrid_and_y_axis_mode() {
        let mut app = test_app();

        handle_key(key(KeyCode::Char('g')), size(), &mut app)
            .await
            .unwrap();
        assert!(!app.autogrid_enabled);

        handle_key(key(KeyCode::Char('y')), size(), &mut app)
            .await
            .unwrap();
        assert_eq!(app.panels[0].y_axis_mode, YAxisMode::ZeroBased);
    }
}
