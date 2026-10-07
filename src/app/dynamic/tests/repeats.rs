use super::*;

#[test]
fn repeat_reorder_preserves_duplicate_identity_focus_data_and_body_offset() {
    use crate::app::panel_scroll::{PanelBodyIdentity, PanelBodyScroll};
    let mut app = repeated_app(&["a", "b"]);
    assert_eq!(
        app.panels
            .iter()
            .map(|p| p.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Load A", "Load B", "Summary"]
    );
    app.selected_item = Some(DashboardItemId::Panel(1));
    app.panels[1].series = vec![SeriesView {
        name: "kept".into(),
        value: Some(9.0),
        points: vec![(1000., 9.)],
        visible: true,
    }];
    app.panel_body_scroll.insert(
        1,
        PanelBodyScroll {
            identity: PanelBodyIdentity::Table,
            offset: 7,
        },
    );
    let variable = &mut app.variable_state.scopes[0].variables[0];
    variable.values = vec!["b".into(), "a".into(), "b".into()];
    variable.texts = vec!["B".into(), "A".into(), "B".into()];
    app.reconcile_dynamic();
    assert_eq!(
        app.panels
            .iter()
            .map(|p| p.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Load B", "Load A", "Load B", "Summary"]
    );
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
    assert_eq!(app.panels[0].series[0].value, Some(9.));
    assert!(app.panels[2].series.is_empty());
    assert_eq!(app.panel_body_scroll[&0].offset, 7);
    assert!(!app.panel_body_scroll.contains_key(&2));
    let variable = &mut app.variable_state.scopes[0].variables[0];
    variable.values = vec!["b".into()];
    variable.texts = vec!["B".into()];
    app.reconcile_dynamic();
    assert_eq!(app.panels.len(), 2);
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
    assert_eq!(app.panel_body_scroll.len(), 1);
}
#[test]
fn repeat_all_and_empty_options_use_concrete_values_and_placeholder() {
    let mut app = repeated_app(&["$__all"]);
    app.variable_state.scopes[0].variables[0].all = true;
    app.reconcile_dynamic();
    assert_eq!(
        app.panels
            .iter()
            .map(|p| p.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Load A", "Load B", "Summary"]
    );
    app.variable_state.scopes[0].variables[0].options.clear();
    app.reconcile_dynamic();
    assert_eq!(app.panels[0].title, "Load All");
    assert_eq!(app.panels.len(), 2);
    app.variable_state.scopes[0].variables.clear();
    app.reconcile_dynamic();
    assert_eq!(app.panels[0].title, "Load ");
}
#[tokio::test]
async fn repeat_queries_use_instance_values_without_leaking_to_dashboard() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let mut app = repeated_app(&["a", "b"]);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    app.prometheus =
        crate::prom::PromClient::new(format!("http://{}", listener.local_addr().unwrap()));
    let server = tokio::spawn(async move {
        let mut queries = Vec::new();
        for _ in 0..3 {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = [0; 4096];
            let n = s.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            let url = reqwest::Url::parse(&format!(
                "http://localhost{}",
                req.split_whitespace().nth(1).unwrap()
            ))
            .unwrap();
            queries.push(
                url.query_pairs()
                    .find(|(k, _)| k == "query")
                    .unwrap()
                    .1
                    .to_string(),
            );
            let body = r#"{"status":"success","data":{"resultType":"matrix","result":[]}}"#;
            s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
        }
        queries
    });
    tokio::time::timeout(Duration::from_secs(3), app.refresh())
        .await
        .unwrap()
        .unwrap();
    let mut queries = tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap();
    queries.sort();
    assert_eq!(
        queries,
        vec![
            "up",
            "up{node=\"a\",region=\"west\"}[15s]",
            "up{node=\"b\",region=\"west\"}[15s]"
        ]
    );
    assert_eq!(
        app.variable_state.scopes[0].variables[0].values,
        vec!["a", "b"]
    );
}

#[tokio::test]
async fn repeat_buffers_follow_reconciliation_and_resize() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::{Rect, Size};
    let mut app = repeated_app(&["a", "b"]);
    app.view_end_ts = 1_783_080_000;
    for (index, panel) in app.panels.iter_mut().take(2).enumerate() {
        panel.series = (1..=20)
            .map(|n| SeriesView {
                name: format!("{}-row{n:02}", if index == 0 { "a" } else { "b" }),
                value: Some(n as f64),
                points: vec![],
                visible: true,
            })
            .collect();
    }
    let initial = draw(&mut app, 120, 32);
    assert!(buffer_text(&initial).contains("Load A"));
    assert!(buffer_text(&initial).contains("Load B"));
    crate::app::input::handle_key(
        KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        Size::new(120, 32),
        &mut app,
    )
    .await
    .unwrap();
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(1)));
    let focused = draw(&mut app, 120, 32);
    let selected = crate::ui::visible_dashboard_rects(Rect::new(0, 0, 120, 32), &app)
        .into_iter()
        .find(|r| r.id == DashboardItemId::Panel(1))
        .unwrap();
    assert_eq!(
        focused[(selected.rect.x, selected.rect.y)].fg,
        ratatui::style::Color::Cyan
    );
    crate::app::input::handle_key(
        KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL),
        Size::new(120, 32),
        &mut app,
    )
    .await
    .unwrap();
    assert_eq!(app.panel_body_scroll[&1].offset, 1);
    let scrolled = draw(&mut app, 120, 32);
    assert!(!buffer_text(&scrolled).contains("b-row01"));
    assert!(buffer_text(&scrolled).contains("b-row02"));
    let variable = &mut app.variable_state.scopes[0].variables[0];
    variable.values = vec!["b".into(), "a".into()];
    variable.texts = vec!["B".into(), "A".into()];
    app.reconcile_dynamic();
    assert_eq!(app.selected_panel_index(), Some(0));
    assert_eq!(app.panel_body_scroll[&0].offset, 1);
    let reordered = draw(&mut app, 120, 32);
    assert!(buffer_text(&reordered).contains("b-row02"));
    let variable = &mut app.variable_state.scopes[0].variables[0];
    variable.values = vec!["b".into()];
    variable.texts = vec!["B".into()];
    app.reconcile_dynamic();
    let removed = draw(&mut app, 120, 32);
    assert!(!buffer_text(&removed).contains("Load A"));
    let narrow = draw(&mut app, 40, 20);
    assert!(buffer_text(&narrow).contains("Load B"));
    assert!(!buffer_text(&narrow).contains("Load A"));
    let svg = crate::export::render_svg(&app, Rect::new(0, 0, 40, 20));
    assert!(svg.contains("Load B"));
    assert!(!svg.contains("Load A"));
}
#[test]
fn repeat_recording_tracks_instance_changes_and_noops() {
    let mut app = repeated_app(&["a", "b"]);
    let dir =
        std::env::temp_dir().join(format!("grafatui-dynamic-recording-{}", std::process::id()));
    app.export.dir = dir.clone();
    let area = ratatui::layout::Rect::new(0, 0, 120, 32);
    crate::export::toggle_recording(&mut app, area).unwrap();
    assert_eq!(app.recording.as_ref().unwrap().frames.len(), 1);
    app.reconcile_dynamic();
    crate::export::capture_recording_frame(&mut app, area).unwrap();
    assert_eq!(app.recording.as_ref().unwrap().frames.len(), 1);
    let v = &mut app.variable_state.scopes[0].variables[0];
    v.values = vec!["b".into()];
    v.texts = vec!["B".into()];
    app.reconcile_dynamic();
    crate::export::capture_recording_frame(&mut app, area).unwrap();
    assert_eq!(app.recording.as_ref().unwrap().frames.len(), 2);
    crate::export::stop_recording(&mut app, crate::export::RecordingCompletionReason::Stopped)
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}
