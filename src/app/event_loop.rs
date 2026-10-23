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
use super::state::{AppState, DashboardWorkspace};
use crate::export::{self, RecordingCompletionReason};
use crate::ui;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::Terminal;
use ratatui::layout::Rect;
use std::time::Duration;

pub(crate) async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    workspace: &mut DashboardWorkspace,
    _tick_rate: Duration,
) -> Result<()>
where
    <B as ratatui::backend::Backend>::Error: Send + Sync + 'static,
{
    let content_viewport = ui::dashboard_viewport(
        terminal_viewport(terminal)?,
        workspace.dashboards().len() > 1,
    );
    super::panel_scroll::reconcile_panel_body_scroll(content_viewport, workspace.active_mut());
    let mut needs_draw = true;

    loop {
        let viewport = terminal_viewport(terminal)?;
        let content_viewport = ui::dashboard_viewport(viewport, workspace.dashboards().len() > 1);
        let completed = workspace.collect_completed_refreshes().await?;
        for (index, before_refresh) in &completed {
            let dashboard = workspace.dashboard_mut(*index);
            reconcile_after_refresh(content_viewport, dashboard, *before_refresh);
            capture_recording_after_change(terminal, dashboard)?;
            if *index == workspace.active_index() {
                needs_draw = true;
            }
        }
        workspace.start_due_refreshes();

        if needs_draw {
            terminal.draw(|f| ui::draw_workspace_ui(f, workspace))?;
            needs_draw = false;
        }

        // `event::poll` blocks this task, so it is kept short and the loop
        // yields below; otherwise shutdown signals, which `main` selects on
        // alongside this loop, would wait for the next refresh.
        let timeout = workspace.next_refresh_in().min(MAX_INPUT_WAIT);

        if event::poll(timeout)? {
            let action = handle_event(event::read()?, viewport, workspace).await?;
            match action {
                InputAction::Quit => {
                    workspace.finalize_recordings()?;
                    return Ok(());
                }
                InputAction::ExportCurrent => {
                    let viewport = terminal_viewport(terminal)?;
                    let app = workspace.active_mut();
                    let exported = export::export_current(app, viewport);
                    report_export_error(app, "Export", exported);
                    needs_draw = true;
                    capture_recording_after_change(terminal, app)?;
                }
                InputAction::ToggleRecording => {
                    let viewport = terminal_viewport(terminal)?;
                    let app = workspace.active_mut();
                    let toggled = export::toggle_recording(app, viewport);
                    report_export_error(app, "Recording", toggled);
                    needs_draw = true;
                    capture_recording_after_change(terminal, app)?;
                }
                InputAction::Refresh | InputAction::Activate(_) => {
                    needs_draw = true;
                    capture_recording_after_change(terminal, workspace.active_mut())?;
                }
                InputAction::Redraw => {
                    needs_draw = true;
                    capture_recording_after_change(terminal, workspace.active_mut())?;
                }
            }
        }

        workspace.start_due_refreshes();
        tokio::task::yield_now().await;
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
    super::panel_scroll::reconcile_panel_body_scroll(viewport, app);
}

/// Longest the event loop blocks waiting for input before yielding.
const MAX_INPUT_WAIT: Duration = Duration::from_millis(250);

pub(super) async fn handle_event(
    event: Event,
    viewport: Rect,
    workspace: &mut DashboardWorkspace,
) -> Result<InputAction> {
    let viewport = match &event {
        Event::Resize(width, height) => Rect::new(0, 0, *width, *height),
        _ => viewport,
    };
    let content_viewport = ui::dashboard_viewport(viewport, workspace.dashboards().len() > 1);
    let action = match event {
        Event::Key(key) if key.code == KeyCode::Tab => {
            workspace.switch_next();
            InputAction::Redraw
        }
        Event::Key(key) if key.code == KeyCode::BackTab => {
            workspace.switch_previous();
            InputAction::Redraw
        }
        Event::Key(key) => {
            input::handle_key_in_viewport(key, content_viewport, workspace.active_mut()).await?
        }
        Event::Mouse(mouse) => {
            if content_viewport.contains(ratatui::layout::Position::new(mouse.column, mouse.row)) {
                input::handle_mouse_in_viewport(mouse, content_viewport, workspace.active_mut())
                    .await?
            } else {
                InputAction::Redraw
            }
        }
        Event::Resize(_, _) => {
            ui::scroll_selected_into_view(content_viewport, workspace.active_mut());
            InputAction::Redraw
        }
        _ => InputAction::Redraw,
    };
    match &action {
        InputAction::Refresh => workspace.request_active_refresh(),
        InputAction::Activate(indices) => workspace.request_activation_refresh(indices.clone()),
        _ => {}
    }
    super::panel_scroll::reconcile_panel_body_scroll(content_viewport, workspace.active_mut());
    Ok(action)
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
    let captured = export::capture_recording_frame(app, viewport);
    report_export_error(app, "Recording frame", captured);
    Ok(())
}

/// Shows a failed export or recording step in the status line instead of
/// ending the session, so a full disk or a read-only directory is recoverable.
fn report_export_error<T>(app: &mut AppState, action: &str, result: Result<T>) {
    if let Err(error) = result {
        app.export_status = Some(format!("{action} failed: {error:#}"));
    }
}

/// Saves an active recording's manifest. It is a no-op without a recording, so
/// it is safe to call on every exit path.
pub(crate) fn finalize_recording_before_quit(app: &mut AppState) -> Result<()> {
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
                    query_index: 0,
                    name: "usage".to_string(),
                    display_name: None,
                    value: Some(1.0),
                    points: vec![(now - 100.0, 0.0), (now, 1.0)],
                    visible: true,
                }],
                last_error: None,
                notices: Default::default(),
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
                display_name_template: None,
                options: PanelOptions::Graph(GraphOptions::default()),
                resolution: Default::default(),
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
                        query_index: 0,
                        name: format!("row{:02}", j + 1),
                        display_name: None,
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

    #[tokio::test]
    async fn dashboard_switch_preserves_saved_scroll_with_distinct_refresh_times() {
        use crossterm::event::{KeyEvent, KeyModifiers};
        let alpha = test_app(ExportOptions::default());
        let mut beta = content_fit_test_app();
        beta.last_refresh = std::time::Instant::now();
        beta.vertical_scroll = 1;
        let viewport = Rect::new(0, 0, 100, 24);
        ui::clamp_dashboard_scroll(ui::dashboard_viewport(viewport, true), &mut beta);
        assert_eq!(
            beta.vertical_scroll, 1,
            "fixture must have a valid saved scroll position"
        );
        let mut workspace = DashboardWorkspace::new(vec![alpha, beta]);
        for code in [KeyCode::Tab, KeyCode::BackTab, KeyCode::Tab] {
            handle_event(
                Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
                viewport,
                &mut workspace,
            )
            .await
            .unwrap();
        }
        assert_eq!(workspace.active_index(), 1);
        assert_eq!(workspace.active().vertical_scroll, 1);
    }

    #[tokio::test]
    async fn workspace_row_input_matches_rendered_geometry() {
        use crate::dashboard::{
            DashboardItemId, DashboardLayout, DashboardLayoutItem, DashboardRow, RowId,
        };
        use crossterm::event::{KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        let mut alpha = test_app(ExportOptions::default());
        alpha.title = "Alpha".into();
        alpha.range = Duration::from_secs(300);
        alpha.view_end_ts = 1_783_080_000;
        alpha.panels[0].title = "Alpha CPU".into();
        alpha.panels[0].series[0].points = vec![(1_783_079_900.0, 0.0), (1_783_080_000.0, 1.0)];
        alpha.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::Row(
            DashboardRow::new(
                RowId::new(0),
                "Parent",
                true,
                false,
                vec![DashboardLayoutItem::Panel(0)],
            ),
        )]));
        let mut beta = test_app(ExportOptions::default());
        beta.title = "Beta".into();
        let mut workspace = DashboardWorkspace::new(vec![alpha, beta]);
        let viewport = Rect::new(0, 0, 80, 24);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let capture = |terminal: &Terminal<TestBackend>, name: &str| {
            if let Ok(directory) = std::env::var("GRAFATUI_MULTI_DASHBOARD_FIX_CAPTURE_DIR") {
                let directory = std::path::PathBuf::from(directory);
                fs::create_dir_all(&directory).unwrap();
                ui::capture_buffer(terminal, &directory.join(format!("{name}-80x24.json")));
            }
        };
        terminal
            .draw(|frame| ui::draw_workspace_ui(frame, &mut workspace))
            .unwrap();
        assert_eq!(
            terminal.backend().buffer().cell((1, 5)).unwrap().symbol(),
            "▶"
        );
        capture(&terminal, "row-collapsed");
        let action = handle_event(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 5,
                modifiers: KeyModifiers::NONE,
            }),
            viewport,
            &mut workspace,
        )
        .await
        .unwrap();
        assert_eq!(action, InputAction::Activate(vec![0]));
        assert!(
            !workspace
                .active()
                .layout
                .row(RowId::new(0))
                .unwrap()
                .collapsed
        );
        terminal
            .draw(|frame| ui::draw_workspace_ui(frame, &mut workspace))
            .unwrap();
        assert_eq!(
            terminal.backend().buffer().cell((1, 5)).unwrap().symbol(),
            "▼"
        );
        capture(&terminal, "row-expanded");
        handle_event(
            Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            viewport,
            &mut workspace,
        )
        .await
        .unwrap();
        assert_eq!(workspace.active().selected_panel_index(), Some(0));
        terminal
            .draw(|frame| ui::draw_workspace_ui(frame, &mut workspace))
            .unwrap();
        let rect =
            ui::visible_dashboard_rects(ui::dashboard_viewport(viewport, true), workspace.active())
                .into_iter()
                .find(|item| item.id == DashboardItemId::Panel(0))
                .unwrap()
                .rect;
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((rect.x, rect.y))
                .unwrap()
                .fg,
            workspace.active().theme.border_selected
        );
        capture(&terminal, "row-panel-focused");
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
            let action = input::handle_key(
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
                size,
                &mut app,
            )
            .await
            .unwrap();
            assert_eq!(action, InputAction::Refresh);
            app.refresh().await.unwrap();
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

    #[tokio::test]
    async fn bounds_scroll_manual_and_periodic_refresh_clamp_real_data() {
        use crate::{
            app::QueryMode,
            app::panel_scroll::{PanelBodyIdentity, PanelBodyScroll},
            dashboard::{DashboardItemId, DashboardLayoutItem},
        };
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for count in [1, 20] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut bytes = [0; 4096];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = socket.read(&mut bytes).await.unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&bytes[..n]);
                }
                assert!(String::from_utf8(request).unwrap().contains("query=B&"));
                let result=(1..=count).map(|n|serde_json::json!({"metric":{"__name__":format!("row{n:02}")},"value":[1783080000,n.to_string()]})).collect::<Vec<_>>();
                let body=serde_json::json!({"status":"success","data":{"resultType":"vector","result":result}}).to_string();
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
        });
        let mut app = content_fit_test_app();
        app.panels = vec![app.panels[1].clone()];
        app.panels[0].exprs = vec!["B".into()];
        app.panels[0].legends = vec![Some("{{__name__}}".into())];
        app.panels[0].query_modes = vec![QueryMode::Instant];
        let DashboardLayoutItem::AutoGrid(g) = &mut app.layout.items[0] else {
            panic!()
        };
        g.options.max_height = Some(10);
        g.items = crate::dashboard::autogrid::test_items(vec![0]);
        app.selected_item = Some(DashboardItemId::Panel(0));
        app.panel_body_scroll.insert(
            0,
            PanelBodyScroll {
                identity: PanelBodyIdentity::Table,
                offset: 14,
            },
        );
        app.prometheus = PromClient::new(format!("http://{address}"));
        let viewport = Rect::new(0, 0, 100, 40);
        for periodic in [false, true] {
            let before = app.last_refresh;
            if periodic {
                app.refresh().await.unwrap();
            } else {
                let action = input::handle_key(
                    KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
                    ratatui::layout::Size::new(100, 40),
                    &mut app,
                )
                .await
                .unwrap();
                assert_eq!(action, InputAction::Refresh);
                app.refresh().await.unwrap();
            }
            reconcile_after_refresh(viewport, &mut app, before);
            assert_eq!(app.panels[0].series.len(), if periodic { 20 } else { 1 });
            assert_eq!(app.panel_body_scroll[&0].offset, 0);
            assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
            assert!(app.panels[0].last_error.is_none());
        }
        server.await.unwrap();
    }

    #[test]
    fn export_failures_are_reported_instead_of_ending_the_session() {
        // A file where the export directory should be makes every write fail.
        let blocker = test_export_dir("blocked");
        fs::write(&blocker, "not a directory").unwrap();
        let mut app = test_app(ExportOptions {
            dir: blocker.clone(),
            format: ExportFormat::Svg,
            record_max_frames: 10,
        });

        let exported = export::export_current(&mut app, Rect::new(0, 0, 100, 40));
        report_export_error(&mut app, "Export", exported);
        let toggled = export::toggle_recording(&mut app, Rect::new(0, 0, 100, 40));
        report_export_error(&mut app, "Recording", toggled);

        let status = app.export_status.clone().unwrap();
        assert!(status.starts_with("Recording failed: "), "{status}");
        assert!(app.recording.is_none());
        fs::remove_file(blocker).unwrap();
    }

    #[test]
    fn finalizing_saves_an_active_recording_once() {
        let dir = test_export_dir("finalize");
        let mut app = test_app(ExportOptions {
            dir: dir.clone(),
            format: ExportFormat::Svg,
            record_max_frames: 10,
        });
        export::toggle_recording(&mut app, Rect::new(0, 0, 100, 40)).unwrap();
        let recording_dir = app.recording.as_ref().unwrap().dir.clone();

        finalize_recording_before_quit(&mut app).unwrap();
        finalize_recording_before_quit(&mut app).unwrap();

        assert!(app.recording.is_none());
        assert!(recording_dir.join("manifest.json").is_file());
        fs::remove_dir_all(dir).unwrap();
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
