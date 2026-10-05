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
    terminal.draw(|f| crate::ui::draw_ui(f, app)).unwrap();
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
