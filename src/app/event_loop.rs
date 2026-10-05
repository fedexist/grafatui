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
            let before_refresh = app.last_refresh;
            let action = match event::read()? {
                Event::Key(key) => {
                    let size = terminal.size()?;
                    input::handle_key(key, size, app).await?
                }
                Event::Mouse(mouse) => {
                    let size = terminal.size()?;
                    input::handle_mouse(mouse, size, app).await?
                }
                Event::Resize(width, height) => {
                    ui::scroll_selected_into_view(Rect::new(0, 0, width, height), app);
                    InputAction::Redraw
                }
                _ => InputAction::Redraw,
            };

            reconcile_after_refresh(terminal_viewport(terminal)?, app, before_refresh);
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
            let before_refresh = app.last_refresh;
            app.refresh().await?;
            reconcile_after_refresh(terminal_viewport(terminal)?, app, before_refresh);
            needs_draw = true;
            capture_recording_after_change(terminal, app)?;
        }
    }
}

fn reconcile_after_refresh(viewport: Rect, app: &mut AppState, before: std::time::Instant) {
    if app.last_refresh != before
        && matches!(
            app.mode,
            crate::app::AppMode::Normal | crate::app::AppMode::Inspect
        )
    {
        ui::scroll_selected_into_view(viewport, app);
        ui::clamp_dashboard_scroll(viewport, app);
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

    fn content_fit_test_app() -> AppState {
        use crate::dashboard::autogrid::{AutoGridOptions, DashboardAutoGrid, test_items};
        use crate::dashboard::{DashboardLayout, DashboardLayoutItem};
        let mut app = test_app(ExportOptions::default());
        let template = app.panels[0].clone();
        app.panels = (0..4)
            .map(|i| {
                let mut p = template.clone();
                p.title = ["A", "B", "C", "D"][i].into();
                p.panel_type = if i == 2 {
                    PanelType::Stat
                } else {
                    PanelType::Table
                };
                p.series = (0..[3, 20, 1, 0][i])
                    .map(|j| SeriesView {
                        name: format!("row{:02}", j + 1),
                        value: Some((j + 1) as f64),
                        points: vec![],
                        visible: true,
                    })
                    .collect();
                p
            })
            .collect();
        app.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::AutoGrid(
            DashboardAutoGrid {
                options: AutoGridOptions {
                    fit_content: true,
                    min_height: Some(0),
                    match_row_heights: false,
                    ..Default::default()
                },
                items: test_items(vec![0, 1, 2, 3]),
            },
        )]));
        app.title = "AutoGrid".into();
        app.view_end_ts = 1_783_080_000;
        app
    }

    #[test]
    fn content_fit_refresh_reconciles_scroll() {
        use crate::dashboard::DashboardItemId;
        let mut app = content_fit_test_app();
        app.selected_item = Some(DashboardItemId::Panel(3));
        app.vertical_scroll = 1;
        let before = app.last_refresh;
        app.panels[1].series.truncate(1);
        app.last_refresh = std::time::Instant::now();
        reconcile_after_refresh(Rect::new(0, 0, 100, 48), &mut app, before);
        assert_eq!(app.selected_item, Some(DashboardItemId::Panel(3)));
        assert_eq!(app.vertical_scroll, 0);
        // No completed refresh: manual scrolling must survive the event boundary.
        app.vertical_scroll = 1;
        let before = app.last_refresh;
        reconcile_after_refresh(Rect::new(0, 0, 100, 48), &mut app, before);
        assert_eq!(app.vertical_scroll, 1);
        // Fullscreen has its own presentation and keeps its dashboard position.
        app.mode = crate::app::AppMode::Fullscreen;
        app.last_refresh = std::time::Instant::now();
        reconcile_after_refresh(Rect::new(0, 0, 100, 48), &mut app, before);
        assert_eq!(app.vertical_scroll, 1);
    }

    #[tokio::test]
    async fn content_fit_manual_refresh_keeps_focus_through_data_changes() {
        use crate::app::QueryMode;
        use crate::dashboard::DashboardItemId;
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for b_rows in [1, 20] {
                for _ in 0..4 {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let mut request = Vec::new();
                    let mut chunk = [0; 4096];
                    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                        let n = socket.read(&mut chunk).await.unwrap();
                        assert!(n > 0);
                        request.extend_from_slice(&chunk[..n]);
                    }
                    let request = String::from_utf8(request).unwrap();
                    let count = if request.contains("query=B&") {
                        b_rows
                    } else if request.contains("query=A&") {
                        3
                    } else if request.contains("query=C&") {
                        1
                    } else {
                        0
                    };
                    let result=(1..=count).map(|i|serde_json::json!({"metric":{"__name__":format!("row{i:02}")},"value":[1783080000,i.to_string()]})).collect::<Vec<_>>();
                    let body=serde_json::json!({"status":"success","data":{"resultType":"vector","result":result}}).to_string();
                    socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
                }
            }
        });
        let mut app = content_fit_test_app();

        for p in &mut app.panels {
            p.exprs = vec![p.title.clone()];
            p.legends = vec![Some("{{__name__}}".into())];
            p.query_modes = vec![QueryMode::Instant];
        }
        let size = ratatui::layout::Size::new(100, 24);
        let viewport = Rect::new(0, 0, 100, 24);
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        terminal.draw(|f| ui::draw_ui(f, &mut app)).unwrap();
        for _ in 0..3 {
            input::handle_key(
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                size,
                &mut app,
            )
            .await
            .unwrap();
            terminal.draw(|f| ui::draw_ui(f, &mut app)).unwrap();
        }
        assert_eq!(app.selected_item, Some(DashboardItemId::Panel(3)));
        for count in [1, 20] {
            app.prometheus = PromClient::new(format!("http://{address}"));
            let before = app.last_refresh;
            input::handle_key(
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
                size,
                &mut app,
            )
            .await
            .unwrap();
            reconcile_after_refresh(viewport, &mut app, before);
            assert_eq!(app.panels[1].series.len(), count);
            assert!(app.panels.iter().all(|p| p.last_error.is_none()));
            assert_eq!(app.selected_item, Some(DashboardItemId::Panel(3)));
            assert!(
                ui::visible_dashboard_rects(viewport, &app)
                    .iter()
                    .any(|r| r.id == DashboardItemId::Panel(3))
            );
            app.view_end_ts = 1_783_080_000;
            app.prometheus = PromClient::new("http://localhost:9090".into());
            terminal.draw(|f| ui::draw_ui(f, &mut app)).unwrap();
        }
        ui::scroll_selected_into_view(Rect::new(0, 0, 140, 48), &mut app);
        assert_eq!(app.selected_item, Some(DashboardItemId::Panel(3)));
        let mut resized = Terminal::new(TestBackend::new(140, 48)).unwrap();
        resized.draw(|f| ui::draw_ui(f, &mut app)).unwrap();
        let text = resized
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        for title in ["A", "B", "C", "D"] {
            assert!(text.contains(&format!("┌{title}")));
        }
        server.await.unwrap();
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
    fn test_capture_recording_after_annotation_change_writes_frame() {
        let dir = test_export_dir("annotation-recording");
        let export = ExportOptions {
            dir: dir.clone(),
            format: ExportFormat::Svg,
            record_max_frames: 10,
        };
        let mut app = test_app(export);
        let backend = TestBackend::new(100, 40);
        let terminal = Terminal::new(backend).unwrap();

        export::toggle_recording(&mut app, Rect::new(0, 0, 100, 40)).unwrap();
        app.annotations = crate::annotations::AnnotationState::from_events_for_test(vec![
            crate::annotations::test_event_at(app.view_end_ts as f64, "deploy"),
        ]);

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
