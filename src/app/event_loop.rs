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
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::Terminal;
use std::time::Duration;

enum SharedKeyResult {
    Handled,
    Quit,
    Unhandled,
}

async fn handle_shared_keys(key: event::KeyEvent, app: &mut AppState) -> Result<SharedKeyResult> {
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

pub async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut AppState,
    _tick_rate: Duration,
) -> Result<()>
where
    <B as ratatui::backend::Backend>::Error: Send + Sync + 'static,
{
    let mut needs_draw = true;

    loop {
        if needs_draw {
            terminal.draw(|f| ui::draw_ui(f, app))?;
            needs_draw = false;
        }

        let timeout = app.refresh_every.saturating_sub(app.last_refresh.elapsed());

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if app.mode == AppMode::Search {
                        match key.code {
                            KeyCode::Esc => {
                                app.mode = AppMode::Normal;
                                app.search_query.clear();
                                app.search_results.clear();
                            }
                            KeyCode::Enter => {
                                if let Some(&idx) = app.search_results.first() {
                                    app.selected_panel = idx;
                                    app.mode = AppMode::Fullscreen; // Go to Fullscreen on selection
                                    app.search_query.clear();
                                    app.search_results.clear();
                                }
                            }
                            KeyCode::Backspace => {
                                app.search_query.pop();
                                if app.search_query.is_empty() {
                                    app.search_results.clear();
                                } else {
                                    app.search_results = app
                                        .panels
                                        .iter()
                                        .enumerate()
                                        .filter(|(_, p)| {
                                            p.title
                                                .to_lowercase()
                                                .contains(&app.search_query.to_lowercase())
                                        })
                                        .map(|(i, _)| i)
                                        .collect();
                                }
                            }
                            KeyCode::Char(c) => {
                                app.search_query.push(c);
                                app.search_results = app
                                    .panels
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, p)| {
                                        p.title
                                            .to_lowercase()
                                            .contains(&app.search_query.to_lowercase())
                                    })
                                    .map(|(i, _)| i)
                                    .collect();
                            }
                            _ => {}
                        }
                    } else if app.mode == AppMode::Inspect {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('v') => {
                                app.mode = AppMode::Normal;
                                app.cursor_x = None;
                            }
                            KeyCode::Left => {
                                app.move_cursor(-1);
                            }
                            KeyCode::Right => {
                                app.move_cursor(1);
                            }
                            KeyCode::Char('q') => return Ok(()),
                            _ => {}
                        }
                    } else if app.mode == AppMode::Fullscreen {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('f') | KeyCode::Enter => {
                                app.mode = AppMode::Normal;
                            }
                            KeyCode::Char('v') => {
                                app.mode = AppMode::FullscreenInspect;
                                app.center_cursor();
                            }
                            KeyCode::PageUp => {
                                app.select_previous_panel();
                            }
                            KeyCode::PageDown => {
                                app.select_next_panel();
                            }
                            _ => match handle_shared_keys(key, app).await? {
                                SharedKeyResult::Quit => return Ok(()),
                                SharedKeyResult::Handled | SharedKeyResult::Unhandled => {}
                            },
                        }
                    } else if app.mode == AppMode::FullscreenInspect {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('v') => {
                                app.mode = AppMode::Fullscreen;
                                app.cursor_x = None;
                            }
                            KeyCode::Char('g') => {
                                app.autogrid_enabled = !app.autogrid_enabled;
                            }
                            KeyCode::Left => {
                                app.move_cursor(-1);
                            }
                            KeyCode::Right => {
                                app.move_cursor(1);
                            }
                            KeyCode::Char('q') => return Ok(()),
                            _ => {}
                        }
                    } else {
                        // Normal Mode
                        match key.code {
                            KeyCode::Char('f') => {
                                app.mode = AppMode::Fullscreen;
                            }
                            KeyCode::Char('v') => {
                                app.mode = AppMode::Inspect;
                                app.center_cursor();
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                app.select_previous_panel();
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                app.select_next_panel();
                            }
                            KeyCode::PageUp => {
                                app.vertical_scroll = app.vertical_scroll.saturating_sub(10);
                            }
                            KeyCode::PageDown => {
                                app.vertical_scroll = app.vertical_scroll.saturating_add(10);
                            }
                            KeyCode::Char(c) if c.is_ascii_digit() => {
                                if let Some(digit) = c.to_digit(10) {
                                    if let Some(panel) = app.panels.get_mut(app.selected_panel) {
                                        if digit == 0 {
                                            // Show all
                                            for s in &mut panel.series {
                                                s.visible = true;
                                            }
                                        } else {
                                            // Toggle specific series (1-based index)
                                            let idx = (digit - 1) as usize;
                                            if let Some(series) = panel.series.get_mut(idx) {
                                                series.visible = !series.visible;
                                            }
                                        }
                                    }
                                }
                            }
                            KeyCode::Home => {
                                app.vertical_scroll = 0;
                            }
                            KeyCode::End => {
                                app.vertical_scroll = usize::MAX; // Will be clamped by rendering logic usually, or we should track max height
                            }
                            KeyCode::Char('?') => {
                                app.debug_bar = !app.debug_bar;
                            }
                            KeyCode::Char('/') => {
                                app.mode = AppMode::Search;
                                app.search_query.clear();
                                app.search_results.clear();
                            }
                            _ => match handle_shared_keys(key, app).await? {
                                SharedKeyResult::Quit => return Ok(()),
                                SharedKeyResult::Handled | SharedKeyResult::Unhandled => {}
                            },
                        }
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
                    | crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left) =>
                    {
                        let size = terminal.size()?;
                        let rect = ratatui::layout::Rect::new(0, 0, size.width, size.height);
                        if let Some((idx, panel_rect)) =
                            ui::hit_test(app, rect, mouse.column, mouse.row)
                        {
                            app.selected_panel = idx;

                            // If in Fullscreen or FullscreenInspect, we are already focused on this panel (effectively)
                            // If in Normal/Inspect, we switch to Inspect mode if not already

                            match app.mode {
                                AppMode::Normal | AppMode::Inspect => {
                                    // In normal mode, mouse clicks only select the panel
                                    // Cursor mode must be activated with 'v' key
                                    // Don't transition to Inspect mode
                                }
                                AppMode::Fullscreen | AppMode::FullscreenInspect => {
                                    // In fullscreen, mouse clicks enable cursor
                                    app.mode = AppMode::FullscreenInspect;

                                    // Calculate cursor_x based on click position within panel_rect
                                    let chart_width = panel_rect.width.saturating_sub(2) as f64;
                                    if chart_width > 0.0 {
                                        let relative_x =
                                            (mouse.column.saturating_sub(panel_rect.x + 1)) as f64;
                                        let fraction = (relative_x / chart_width).clamp(0.0, 1.0);

                                        let (start_ts, _) = app.time_bounds();

                                        app.cursor_x =
                                            Some(start_ts + fraction * app.range.as_secs_f64());
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    crossterm::event::MouseEventKind::ScrollDown => {
                        app.vertical_scroll = app.vertical_scroll.saturating_add(1);
                    }
                    crossterm::event::MouseEventKind::ScrollUp => {
                        app.vertical_scroll = app.vertical_scroll.saturating_sub(1);
                    }
                    _ => {}
                },
                _ => {}
            }

            needs_draw = true;
        }

        if app.last_refresh.elapsed() >= app.refresh_every {
            app.refresh().await?;
            needs_draw = true;
        }
    }
}
