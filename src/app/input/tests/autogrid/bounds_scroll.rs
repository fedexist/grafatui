use super::*;
use crate::dashboard::autogrid::AutoGridOptions;

fn bounded_app() -> AppState {
    let mut app = super::content_fit::content_fit_app();
    let DashboardLayoutItem::AutoGrid(group) = &mut app.layout.items[0] else {
        panic!()
    };
    group.options.max_height = Some(10);
    app
}
fn rows(n: usize) -> Vec<SeriesView> {
    (1..=n)
        .map(|n| SeriesView {
            name: format!("row{n:02}"),
            value: Some(n as f64),
            points: vec![],
            visible: true,
        })
        .collect()
}
fn single_table() -> AppState {
    let mut app = bounded_app();
    app.panels.truncate(1);
    app.panels[0].series = rows(20);
    let DashboardLayoutItem::AutoGrid(group) = &mut app.layout.items[0] else {
        panic!()
    };
    group.items.truncate(1);
    app
}
fn text(buffer: &ratatui::buffer::Buffer) -> String {
    buffer.content.iter().map(|c| c.symbol()).collect()
}
fn render(app: &mut AppState, size: Size, label: &str) -> ratatui::buffer::Buffer {
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(size.width, size.height))
            .unwrap();
    // Fixtures query a local server; presentation uses a stable endpoint label.
    let client = std::mem::replace(
        &mut app.prometheus,
        crate::prom::PromClient::new("http://localhost:9090".into()),
    );
    terminal.draw(|f| crate::ui::draw_ui(f, app)).unwrap();
    app.prometheus = client;
    let buffer = terminal.backend().buffer().clone();
    for item in crate::ui::visible_dashboard_rects(Rect::new(0, 0, size.width, size.height), app) {
        if app.selected_item == Some(item.id)
            && matches!(item.kind, crate::ui::DashboardRectKind::Panel { .. })
        {
            assert_eq!(
                buffer.cell((item.rect.x, item.rect.y)).unwrap().fg,
                app.theme.border_selected,
                "{label}"
            );
        }
    }
    buffer
}

#[test]
fn bounds_scroll_geometry_real_buffers_keep_caps_and_gaps() {
    let mut app = bounded_app();
    let rects = crate::ui::visible_dashboard_rects(Rect::new(0, 0, 100, 48), &app);
    assert_eq!(
        rects
            .iter()
            .map(|r| (r.rect.x, r.rect.y, r.rect.width, r.rect.height))
            .collect::<Vec<_>>(),
        [
            (1, 4, 49, 7),
            (50, 4, 49, 10),
            (1, 14, 49, 18),
            (50, 14, 49, 3)
        ]
    );
    let buffer = render(
        &mut app,
        Size::new(100, 48),
        "bounds-scroll-mixed-unmatched",
    );
    assert!(text(&buffer).contains("row06"));
    assert!(!text(&buffer).contains("row07"));
    assert_eq!(
        crate::ui::hit_test(&app, Rect::new(0, 0, 100, 48), 51, 20),
        None
    );
    for (width, height, fill, matching, heights, ys, label) in [
        (
            100,
            48,
            false,
            true,
            vec![10, 10, 18, 10],
            vec![4, 4, 14, 14],
            "bounds-scroll-mixed-matched",
        ),
        (
            140,
            60,
            true,
            false,
            vec![7, 10, 18, 3],
            vec![4, 4, 4, 31],
            "bounds-scroll-fill-unmatched",
        ),
        (
            140,
            60,
            true,
            true,
            vec![10, 10, 27, 10],
            vec![4, 4, 4, 31],
            "bounds-scroll-fill-matched",
        ),
    ] {
        let DashboardLayoutItem::AutoGrid(g) = &mut app.layout.items[0] else {
            panic!()
        };
        g.options.fill_screen = fill;
        g.options.match_row_heights = matching;
        let rects = crate::ui::visible_dashboard_rects(Rect::new(0, 0, width, height), &app);
        assert_eq!(
            rects.iter().map(|r| r.rect.height).collect::<Vec<_>>(),
            heights
        );
        assert_eq!(rects.iter().map(|r| r.rect.y).collect::<Vec<_>>(), ys);
        let b = render(&mut app, Size::new(width, height), label);
        assert!(text(&b).contains("row06"));
        assert!(!text(&b).contains("row07"));
    }
}

#[test]
fn bounds_scroll_error_document_window_reaches_lower_lines() {
    let mut app = single_table();
    app.panels[0].last_error = Some(
        (0..8)
            .map(|n| format!("line{n:02}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    app.vertical_scroll = 1;
    let buffer = render(&mut app, Size::new(40, 12), "bounds-scroll-error-clipped");
    let txt = text(&buffer);
    assert!(txt.contains("ERROR"));
    assert!(txt.contains("line03"), "{txt}");
    assert!(txt.contains("line05"), "{txt}");
    assert!(!txt.contains("line00"));
}

fn set_offset(app: &mut AppState, offset: u64) {
    use crate::app::panel_scroll::{PanelBodyIdentity, PanelBodyScroll};
    let identity = app.panels[0]
        .last_error
        .clone()
        .map(PanelBodyIdentity::Error)
        .unwrap_or(PanelBodyIdentity::Table);
    app.panel_body_scroll
        .insert(0, PanelBodyScroll { identity, offset });
}

#[test]
fn bounds_scroll_table_local_and_document_windows_keep_headers() {
    let mut app = single_table();
    for (offset, first, last, label) in [
        (0, "row01", "row06", "bounds-scroll-table-start"),
        (1, "row02", "row07", "bounds-scroll-table-down"),
        (7, "row08", "row13", "bounds-scroll-table-page"),
        (14, "row15", "row20", "bounds-scroll-table-end"),
    ] {
        set_offset(&mut app, offset);
        let b = render(&mut app, Size::new(100, 40), label);
        let txt = text(&b);
        assert!(txt.contains(first), "{label}");
        assert!(txt.contains(last), "{label}");
        if offset > 0 {
            assert!(!txt.contains("row01"));
        }
        let cell = b.cell((2, 5)).unwrap();
        assert_eq!(cell.symbol(), "S");
        assert_eq!(cell.fg, app.theme.title);
        assert!(cell.modifier.contains(ratatui::style::Modifier::BOLD));
    }
    let w = crate::ui::panel_body_window(20, 3, 14, 3);
    assert_eq!((w.first, w.length), (17, 3));
    let w = crate::ui::panel_body_window(20, 0, 17, 3);
    assert_eq!((w.first, w.length), (17, 3));
    let w = crate::ui::panel_body_window(20, u64::MAX, u64::MAX, 3);
    assert_eq!((w.first, w.length), (17, 3));
}

#[test]
fn bounds_scroll_fitted_errors_render_wide_windows_and_tiny_caps() {
    let mut app = single_table();
    app.panels[0].last_error = Some(
        (0..70_000)
            .map(|n| format!("line{n:05}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    set_offset(&mut app, u64::MAX);
    let b = render(&mut app, Size::new(100, 40), "bounds-scroll-error-wide-end");
    let txt = text(&b);
    assert!(txt.contains("line69992"));
    assert!(txt.contains("line69999"));
    assert!(!txt.contains("line00000"));
    app.panels[0].last_error = None;
    for cap in [1, 2, 3, 4] {
        let DashboardLayoutItem::AutoGrid(g) = &mut app.layout.items[0] else {
            panic!()
        };
        g.options.max_height = Some(cap);
        let b = render(
            &mut app,
            Size::new(3, 8),
            &format!("bounds-scroll-tiny-{cap}"),
        );
        assert!(!text(&b).contains("row"));
    }
    let DashboardLayoutItem::AutoGrid(g) = &mut app.layout.items[0] else {
        panic!()
    };
    g.options.max_height = Some(1);
    app.mode = AppMode::Fullscreen;
    set_offset(&mut app, 0);
    let b = render(
        &mut app,
        Size::new(100, 40),
        "bounds-scroll-tiny-fullscreen",
    );
    assert!(text(&b).contains("row20"));
    app.mode = AppMode::Normal;
    let b = render(&mut app, Size::new(100, 40), "bounds-scroll-tiny-restored");
    assert!(!text(&b).contains("row01"));
}

#[test]
fn bounds_scroll_table_partial_windows_and_fitted_error_wrap_buffers() {
    let mut app = single_table();
    for (document, local, label) in [
        (1, 14, "bounds-scroll-table-top-clipped"),
        (0, 17, "bounds-scroll-table-bottom-clipped"),
    ] {
        app.vertical_scroll = document;
        set_offset(&mut app, local);
        assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
        let b = render(&mut app, Size::new(100, 14), label);
        let txt = text(&b);
        assert!(txt.contains("row18"));
        assert!(txt.contains("row20"));
        assert!(!txt.contains("row17"));
    }
    app.vertical_scroll = 0;
    app.panel_body_scroll.clear();
    app.panels[0].last_error = Some("one two three four".into());
    let b = render(&mut app, Size::new(12, 24), "bounds-scroll-error-narrow");
    let txt = text(&b);
    assert!(txt.contains("one two"));
    assert!(txt.contains("three"));
    assert!(txt.contains("four"));
    app.panels[0].last_error = Some("Unicode 水é".into());
    let b = render(&mut app, Size::new(42, 24), "bounds-scroll-error-unicode");
    assert!(text(&b).contains("Unicode"));
    assert_eq!(b.cell((10, 5)).unwrap().symbol(), "水");
    assert_eq!(b.cell((12, 5)).unwrap().symbol(), "é");
    for (width, height) in [(0, 0), (0, 8), (3, 0)] {
        render(&mut app, Size::new(width, height), "zero-geometry");
    }
}

fn local(app: &AppState, index: usize) -> u64 {
    app.panel_body_scroll.get(&index).map_or(0, |s| s.offset)
}
async fn control(app: &mut AppState, size: Size, code: KeyCode) {
    handle_key(KeyEvent::new(code, KeyModifiers::CONTROL), size, app)
        .await
        .unwrap();
}
#[tokio::test]
async fn bounds_scroll_clipping_and_input_ownership() {
    let mut app = single_table();
    let size = Size::new(100, 40);
    for (code, offset, first, last, label) in [
        (
            KeyCode::Down,
            1,
            "row02",
            "row07",
            "bounds-scroll-input-down",
        ),
        (
            KeyCode::PageDown,
            7,
            "row08",
            "row13",
            "bounds-scroll-input-page",
        ),
        (
            KeyCode::End,
            14,
            "row15",
            "row20",
            "bounds-scroll-input-end",
        ),
        (
            KeyCode::End,
            14,
            "row15",
            "row20",
            "bounds-scroll-input-end-noop",
        ),
        (
            KeyCode::Home,
            0,
            "row01",
            "row06",
            "bounds-scroll-input-home",
        ),
    ] {
        control(&mut app, size, code).await;
        assert_eq!(local(&app, 0), offset);
        assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
        assert_eq!(app.vertical_scroll, 0);
        let b = render(&mut app, size, label);
        assert!(text(&b).contains(first));
        assert!(text(&b).contains(last));
    }
    app.vertical_scroll = 1;
    control(&mut app, Size::new(100, 14), KeyCode::End).await;
    assert_eq!(local(&app, 0), 14);
    assert_eq!(app.vertical_scroll, 1);
    let b = render(
        &mut app,
        Size::new(100, 14),
        "bounds-scroll-input-clipped-end",
    );
    assert!(text(&b).contains("row18"));
    assert!(text(&b).contains("row20"));
}

#[tokio::test]
async fn bounds_scroll_wheel_boundaries_and_dashboard_fallback() {
    let size = Size::new(100, 24);
    let mouse = |kind, column, row, modifiers| MouseEvent {
        kind,
        column,
        row,
        modifiers,
    };
    let mut app = bounded_app();
    app.selected_item = Some(DashboardItemId::Panel(1));
    handle_mouse(
        mouse(MouseEventKind::ScrollDown, 52, 7, KeyModifiers::NONE),
        size,
        &mut app,
    )
    .await
    .unwrap();
    assert_eq!(local(&app, 1), 1);
    assert_eq!(app.vertical_scroll, 0);
    control(&mut app, size, KeyCode::End).await;
    for _ in 0..3 {
        handle_mouse(
            mouse(MouseEventKind::ScrollDown, 52, 7, KeyModifiers::NONE),
            size,
            &mut app,
        )
        .await
        .unwrap();
    }
    assert_eq!(local(&app, 1), 14);
    assert_eq!(app.vertical_scroll, 0);
    control(&mut app, size, KeyCode::Home).await;
    handle_mouse(
        mouse(MouseEventKind::ScrollUp, 52, 7, KeyModifiers::NONE),
        size,
        &mut app,
    )
    .await
    .unwrap();
    assert_eq!(local(&app, 1), 0);
    assert_eq!(app.vertical_scroll, 0);
    for (x, y, modifiers, label) in [
        (52, 5, KeyModifiers::NONE, "header"),
        (2, 7, KeyModifiers::NONE, "nonselected"),
        (52, 19, KeyModifiers::NONE, "gap"),
        (52, 7, KeyModifiers::SHIFT, "shift"),
    ] {
        app.vertical_scroll = 0;
        handle_mouse(
            mouse(MouseEventKind::ScrollDown, x, y, modifiers),
            size,
            &mut app,
        )
        .await
        .unwrap();
        assert_eq!(app.vertical_scroll, 1, "{label}");
        assert_eq!(local(&app, 1), 0, "{label}");
    }
    app.vertical_scroll = 0;
    control(&mut app, size, KeyCode::Down).await;
    assert_eq!(local(&app, 1), 1);
    handle_key(key(KeyCode::Down), size, &mut app)
        .await
        .unwrap();
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(2)));
    assert_eq!(local(&app, 1), 1);
    render(&mut app, size, "bounds-scroll-focus-next");
    app.vertical_scroll = 0;
    control(&mut app, size, KeyCode::PageDown).await;
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(2)));
    assert_eq!(app.vertical_scroll, 0);
}

fn reconcile(app: &mut AppState, size: Size) {
    crate::app::panel_scroll::reconcile_panel_body_scroll(
        Rect::new(0, 0, size.width, size.height),
        app,
    );
}
#[test]
fn bounds_scroll_reconciles_content_and_visibility() {
    let mut app = single_table();
    let size = Size::new(100, 40);
    set_offset(&mut app, 14);
    app.panels[0].series.truncate(1);
    reconcile(&mut app, size);
    assert_eq!(local(&app, 0), 0);
    render(&mut app, size, "bounds-scroll-shrink");
    app.panels[0].series = rows(20);
    reconcile(&mut app, size);
    assert_eq!(local(&app, 0), 0);
    render(&mut app, size, "bounds-scroll-grow");
    set_offset(&mut app, 14);
    for s in &mut app.panels[0].series[1..] {
        s.visible = false;
    }
    reconcile(&mut app, size);
    assert_eq!(local(&app, 0), 0);
    render(&mut app, size, "bounds-scroll-hidden-series");
    app.panels[0].series = rows(20);
    set_offset(&mut app, 14);
    let error = (0..20)
        .map(|n| format!("error{n:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    app.panels[0].last_error = Some(error.clone());
    reconcile(&mut app, size);
    assert_eq!(local(&app, 0), 0);
    render(&mut app, size, "bounds-scroll-error-reset");
    set_offset(&mut app, 12);
    reconcile(&mut app, size);
    assert_eq!(local(&app, 0), 12);
    render(&mut app, size, "bounds-scroll-error-retained");
    app.panels[0].last_error = Some("different error".into());
    reconcile(&mut app, size);
    assert_eq!(local(&app, 0), 0);
    render(&mut app, size, "bounds-scroll-error-changed");
    app.panels[0].last_error = None;
    reconcile(&mut app, size);
    assert_eq!(local(&app, 0), 0);
    render(&mut app, size, "bounds-scroll-error-recovery");
    set_offset(&mut app, 17);
    reconcile(&mut app, Size::new(100, 14));
    assert_eq!(local(&app, 0), 17);
    reconcile(&mut app, size);
    assert_eq!(local(&app, 0), 14);
    render(&mut app, size, "bounds-scroll-resize-clamp");
    app.panels[0].last_error = Some("abcdefghij".repeat(20));
    reconcile(&mut app, Size::new(12, 24));
    set_offset(&mut app, 17);
    reconcile(&mut app, Size::new(12, 24));
    assert_eq!(local(&app, 0), 17);
    render(
        &mut app,
        Size::new(12, 24),
        "bounds-scroll-narrow-error-end",
    );
    reconcile(&mut app, size);
    assert_eq!(local(&app, 0), 0);
    render(&mut app, size, "bounds-scroll-width-clamp");
    app.panels[0].last_error = None;
    set_offset(&mut app, 14);
    let DashboardLayoutItem::AutoGrid(g) = &mut app.layout.items[0] else {
        panic!()
    };
    g.options.max_height = Some(1);
    reconcile(&mut app, size);
    assert_eq!(local(&app, 0), 14);
    app.panels.clear();
    reconcile(&mut app, size);
    assert!(app.panel_body_scroll.is_empty());
}

#[tokio::test]
async fn bounds_scroll_nested_reactivation_retains_and_clamps_siblings() {
    use crate::dashboard::autogrid::{DashboardAutoGrid, test_items};
    let mut app = bounded_app();
    app.panels.truncate(3);
    for p in &mut app.panels {
        p.panel_type = PanelType::Table;
        p.series = rows(20);
    }
    let grid = |indices| {
        DashboardLayoutItem::AutoGrid(DashboardAutoGrid {
            options: AutoGridOptions {
                fit_content: true,
                min_height: Some(0),
                max_height: Some(10),
                match_row_heights: false,
                ..Default::default()
            },
            items: test_items(indices),
        })
    };
    let row = RowId::new(0);
    let tabs = TabGroupId::new(0);
    app.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::Row(
        DashboardRow::new(
            row,
            "R",
            false,
            false,
            vec![DashboardLayoutItem::Tabs(DashboardTabs::new(
                tabs,
                vec![
                    DashboardTab {
                        title: "First".into(),
                        children: vec![grid(vec![0, 1])],
                    },
                    DashboardTab {
                        title: "Second".into(),
                        children: vec![grid(vec![2])],
                    },
                ],
            ))],
        ),
    )]));
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    app.prometheus =
        crate::prom::PromClient::new(format!("http://{}", listener.local_addr().unwrap()));
    for p in &mut app.panels {
        p.exprs = vec![p.title.clone()];
        p.legends = vec![Some("{{__name__}}".into())];
        p.query_modes = vec![crate::app::QueryMode::Instant];
    }
    let server = tokio::spawn(async move {
        for request_index in 0..5 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut bytes = [0; 4096];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = socket.read(&mut bytes).await.unwrap();
                assert!(n > 0);
                request.extend_from_slice(&bytes[..n]);
            }
            let count =
                if String::from_utf8(request).unwrap().contains("query=A&") || request_index >= 3 {
                    1
                } else {
                    20
                };
            let result=(1..=count).map(|n|serde_json::json!({"metric":{"__name__":format!("row{n:02}")},"value":[1783080000,n.to_string()]})).collect::<Vec<_>>();
            let body=serde_json::json!({"status":"success","data":{"resultType":"vector","result":result}}).to_string();
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
        }
    });
    let size = Size::new(100, 40);
    app.selected_item = Some(DashboardItemId::Panel(0));
    control(&mut app, size, KeyCode::End).await;
    assert_eq!(local(&app, 0), 14);
    app.selected_item = Some(DashboardItemId::Panel(1));
    control(&mut app, size, KeyCode::PageDown).await;
    assert_eq!(local(&app, 1), 6);
    app.selected_item = Some(DashboardItemId::Row(row));
    handle_key(key(KeyCode::Enter), size, &mut app)
        .await
        .unwrap();
    reconcile(&mut app, size);
    render(&mut app, size, "bounds-scroll-nested-collapsed");
    assert_eq!((local(&app, 0), local(&app, 1)), (14, 6));
    app.panels[0].series.truncate(1);
    handle_key(key(KeyCode::Enter), size, &mut app)
        .await
        .unwrap();
    reconcile(&mut app, size);
    assert_eq!((local(&app, 0), local(&app, 1)), (0, 6));
    render(&mut app, size, "bounds-scroll-nested-expanded");
    app.selected_item = Some(DashboardItemId::Tabs(tabs));
    handle_key(key(KeyCode::Right), size, &mut app)
        .await
        .unwrap();
    reconcile(&mut app, size);
    render(&mut app, size, "bounds-scroll-tab-second");
    assert_eq!(local(&app, 1), 6);
    app.panels[1].series.truncate(1);
    handle_key(key(KeyCode::Left), size, &mut app)
        .await
        .unwrap();
    reconcile(&mut app, size);
    assert_eq!(local(&app, 1), 0);
    render(&mut app, size, "bounds-scroll-tab-return-clamped");
    server.await.unwrap();
}

#[tokio::test]
async fn bounds_scroll_modal_inspect_and_fullscreen_precedence() {
    let mut app = single_table();
    let size = Size::new(100, 40);
    app.mode = AppMode::Search;
    control(&mut app, size, KeyCode::Down).await;
    assert_eq!(local(&app, 0), 0);
    render(&mut app, size, "bounds-scroll-search-owner");
    app.mode = AppMode::Normal;
    app.annotations =
        crate::annotations::AnnotationState::from_events_for_test(vec![tagged_event(
            "release", "deploy",
        )]);
    app.open_tag_filter_modal();
    assert!(app.annotation_modal.is_some());
    control(&mut app, size, KeyCode::Down).await;
    assert_eq!(local(&app, 0), 0);
    render(&mut app, size, "bounds-scroll-modal-owner");
    app.annotation_modal = None;
    app.mode = AppMode::Inspect;
    app.cursor_x = Some(1783080000.0);
    control(&mut app, size, KeyCode::Down).await;
    assert_eq!(local(&app, 0), 1);
    let cursor = app.cursor_x;
    handle_key(key(KeyCode::Left), size, &mut app)
        .await
        .unwrap();
    assert_ne!(app.cursor_x, cursor);
    assert_eq!(local(&app, 0), 1);
    assert_eq!(
        handle_key(
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
            size,
            &mut app
        )
        .await
        .unwrap(),
        InputAction::ToggleRecording
    );
    app.mode = AppMode::Normal;
    app.panels[0].series = rows(70_000);
    handle_key(key(KeyCode::Char('f')), size, &mut app)
        .await
        .unwrap();
    reconcile(&mut app, size);
    control(&mut app, size, KeyCode::End).await;
    assert_eq!(local(&app, 0), 69_971);
    let b = render(&mut app, size, "bounds-scroll-fullscreen-wide-end");
    assert!(text(&b).contains("row70000"));
    handle_key(key(KeyCode::Char('f')), size, &mut app)
        .await
        .unwrap();
    reconcile(&mut app, size);
    assert_eq!(local(&app, 0), 69_971);
    render(&mut app, size, "bounds-scroll-fullscreen-return");
    let mut ordinary = bounded_app();
    ordinary.mode = AppMode::Fullscreen;
    ordinary.selected_item = Some(DashboardItemId::Panel(2));
    control(&mut ordinary, size, KeyCode::PageDown).await;
    assert_eq!(ordinary.selected_item, Some(DashboardItemId::Panel(2)));
    handle_key(key(KeyCode::PageDown), size, &mut ordinary)
        .await
        .unwrap();
    assert_eq!(ordinary.selected_item, Some(DashboardItemId::Panel(3)));
}
