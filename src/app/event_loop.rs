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

use super::input::{self, InputAction};
use super::state::AppState;
use crate::ui;
use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::Terminal;
use std::time::Duration;

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
            let action = match event::read()? {
                Event::Key(key) => input::handle_key(key, app).await?,
                Event::Mouse(mouse) => {
                    let size = terminal.size()?;
                    input::handle_mouse(mouse, size, app)?
                }
                _ => InputAction::Redraw,
            };

            match action {
                InputAction::Quit => return Ok(()),
                InputAction::Redraw => needs_draw = true,
            }
        }

        if app.last_refresh.elapsed() >= app.refresh_every {
            app.refresh().await?;
            needs_draw = true;
        }
    }
}
