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
use crate::export::{self, RecordingCompletionReason};
use crate::ui;
use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::Terminal;
use ratatui::layout::Rect;
use std::time::Duration;

pub(crate) async fn run_app<B: ratatui::backend::Backend>(
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
                InputAction::Quit => {
                    finalize_recording_before_quit(app)?;
                    return Ok(());
                }
                InputAction::ExportCurrent => {
                    let viewport = terminal_viewport(terminal)?;
                    export::export_current(app, viewport)?;
                    needs_draw = true;
                    capture_recording_after_change(terminal, app)?;
                }
                InputAction::ToggleRecording => {
                    let viewport = terminal_viewport(terminal)?;
                    export::toggle_recording(app, viewport)?;
                    needs_draw = true;
                    capture_recording_after_change(terminal, app)?;
                }
                InputAction::Redraw => {
                    needs_draw = true;
                    capture_recording_after_change(terminal, app)?;
                }
            }
        }

        if app.last_refresh.elapsed() >= app.refresh_every {
            app.refresh().await?;
            needs_draw = true;
            capture_recording_after_change(terminal, app)?;
        }
    }
}

fn terminal_viewport<B: ratatui::backend::Backend>(terminal: &Terminal<B>) -> Result<Rect>
where
    <B as ratatui::backend::Backend>::Error: Send + Sync + 'static,
{
    let size = terminal.size()?;
    Ok(Rect::new(0, 0, size.width, size.height))
}

fn capture_recording_after_change<B: ratatui::backend::Backend>(
    terminal: &Terminal<B>,
    app: &mut AppState,
) -> Result<()>
where
    <B as ratatui::backend::Backend>::Error: Send + Sync + 'static,
{
    if app.recording.is_none() {
        return Ok(());
    }

    let viewport = terminal_viewport(terminal)?;
    export::capture_recording_frame(app, viewport)
}

fn finalize_recording_before_quit(app: &mut AppState) -> Result<()> {
    if app.recording.is_some() {
        export::stop_recording(app, RecordingCompletionReason::Quit)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{GraphOptions, PanelOptions, PanelState, PanelType, SeriesView, YAxisMode};
    use crate::export::{ExportFormat, ExportOptions};
    use crate::prom::PromClient;
    use crate::theme::Theme;
    use ratatui::backend::TestBackend;
    use std::fs;

    fn test_export_dir(name: &str) -> std::path::PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("grafatui-event-loop-{name}-{suffix}"))
    }

    fn test_app(export: ExportOptions) -> AppState {
        let now = chrono::Utc::now().timestamp() as f64;
        AppState::new(
            PromClient::new("http://localhost:9090".to_string()),
            std::time::Duration::from_secs(100),
            std::time::Duration::from_secs(10),
            std::time::Duration::from_secs(1),
            "test".to_string(),
            vec![PanelState {
                title: "CPU".to_string(),
                exprs: vec![],
                legends: vec![],
                query_modes: vec![],
                series: vec![SeriesView {
                    name: "usage".to_string(),
                    value: Some(1.0),
                    points: vec![(now - 100.0, 0.0), (now, 1.0)],
                    visible: true,
                }],
                last_error: None,
                last_url: None,
                last_samples: 2,
                grid: None,
                y_axis_mode: YAxisMode::Auto,
                panel_type: PanelType::Graph,
                thresholds: None,
                min: None,
                max: None,
                autogrid: None,
                display: crate::ui::DisplayFormat::default(),
                options: PanelOptions::Graph(GraphOptions::default()),
            }],
            0,
            Theme::default(),
            "dashed-line".to_string(),
            export,
        )
    }

    #[test]
    fn test_capture_recording_after_change_writes_changed_refresh_frame() {
        let dir = test_export_dir("recording");
        let export = ExportOptions {
            dir: dir.clone(),
            format: ExportFormat::Svg,
            record_max_frames: 10,
        };
        let mut app = test_app(export);
        let backend = TestBackend::new(100, 40);
        let terminal = Terminal::new(backend).unwrap();

        export::toggle_recording(&mut app, Rect::new(0, 0, 100, 40)).unwrap();
        app.panels[0].series[0].value = Some(2.0);
        app.panels[0].series[0]
            .points
            .push((chrono::Utc::now().timestamp() as f64, 2.0));

        capture_recording_after_change(&terminal, &mut app).unwrap();

        assert_eq!(app.recording.as_ref().unwrap().frame_count, 2);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_finalize_recording_before_quit_writes_manifest() {
        let dir = test_export_dir("quit-recording");
        let export = ExportOptions {
            dir: dir.clone(),
            format: ExportFormat::Svg,
            record_max_frames: 10,
        };
        let mut app = test_app(export);

        export::toggle_recording(&mut app, Rect::new(0, 0, 100, 40)).unwrap();
        finalize_recording_before_quit(&mut app).unwrap();

        assert!(app.recording.is_none());
        let recording_dir = fs::read_dir(&dir).unwrap().next().unwrap().unwrap().path();
        let manifest = recording_dir.join("manifest.json");
        assert!(manifest.exists());
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(manifest).unwrap()).unwrap();
        assert_eq!(json["completed_reason"], "quit");
        fs::remove_dir_all(dir).unwrap();
    }
}
