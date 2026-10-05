use super::*;
use crate::ui::measure_panel_content;

#[test]
fn content_fit_measurement_matches_table_body() {
    let mut app = auto_grid_test_app();
    let p = &mut app.panels[0];
    p.panel_type = PanelType::Table;
    for (n, h) in [(0, 3), (1, 5), (3, 7), (20, 24)] {
        p.series = (1..=n)
            .map(|i| SeriesView {
                name: format!("row{i:02}"),
                value: Some(i as f64),
                points: vec![],
                visible: true,
            })
            .collect();
        assert_eq!(measure_panel_content(p, 40), Some(h));
    }
    for w in [1, 2] {
        assert_eq!(measure_panel_content(p, w), Some(2));
    }
    p.series.truncate(3);
    p.series[0].visible = false;
    p.series[2].visible = false;
    p.series[1].value = None;
    p.display.no_value = Some("missing".into());
    assert_eq!(measure_panel_content(p, 3), Some(5));
    let rows = crate::ui::prepare_table_rows(p);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "row02");
    assert_eq!(rows[0].value, "missing");
    for err in ["first\nsecond", "averylongwordwithnospaces", "Unicode 水é"] {
        p.last_error = Some(err.into());
        assert_eq!(measure_panel_content(p, 40), None);
    }
    for kind in [
        PanelType::Graph,
        PanelType::Unknown,
        PanelType::Stat,
        PanelType::Gauge,
        PanelType::BarGauge,
        PanelType::Heatmap,
    ] {
        p.panel_type = kind;
        p.last_error = None;
        assert_eq!(measure_panel_content(p, 40), None);
        p.series.clear();
        assert_eq!(measure_panel_content(p, 40), None);
        p.last_error = Some("bad".into());
        assert_eq!(measure_panel_content(p, 40), None);
    }
}

fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
    buffer.content.iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn content_fit_measurement_real_buffers_keep_rows_and_errors() {
    use crate::dashboard::autogrid::{AutoGridOptions, DashboardAutoGrid, test_items};
    for (n, h) in [(0, 3), (1, 5), (3, 7), (20, 24)] {
        let mut app = auto_grid_test_app();
        app.panels.truncate(1);
        app.panels[0].panel_type = PanelType::Table;
        app.panels[0].series = (1..=n)
            .map(|i| SeriesView {
                name: format!("row{i:02}"),
                value: Some(i as f64),
                points: vec![],
                visible: true,
            })
            .collect();
        app.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::AutoGrid(
            DashboardAutoGrid {
                options: AutoGridOptions {
                    row_height: h,
                    ..Default::default()
                },
                items: test_items(vec![0]),
            },
        )]));
        assert_eq!(
            measure_panel_content(&app.panels[0], 40),
            Some(u64::from(h))
        );
        let buffer = auto_grid_render(
            &mut app,
            Size::new(40, 40),
            &format!("content-fit-measure-{n}"),
            &["A"],
        );
        let text = buffer_text(&buffer);
        if n == 0 {
            assert!(text.contains("No data"));
        } else {
            assert!(text.contains("Series"));
            assert!(text.contains(&format!("row{n:02}")));
        }
    }
    let mut app = auto_grid_test_app();
    app.panels[0].panel_type = PanelType::Table;
    app.panels[0].last_error = Some("first line\nsecond line Unicode 水é".into());
    assert_eq!(measure_panel_content(&app.panels[0], 40), None);
    let buffer = auto_grid_render(
        &mut app,
        Size::new(40, 24),
        "content-fit-error-baseline",
        &["A"],
    );
    assert!(buffer_text(&buffer).contains("ERROR"));
    assert!(buffer_text(&buffer).contains("first line"));
}

pub(super) fn content_fit_app() -> AppState {
    use crate::dashboard::autogrid::{AutoGridOptions, DashboardAutoGrid, test_items};
    let mut app = auto_grid_test_app();
    for (index, n) in [(0, 3), (1, 20), (3, 0)] {
        app.panels[index].panel_type = PanelType::Table;
        app.panels[index].series = content_rows(n);
    }
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
    app
}

fn content_rows(n: usize) -> Vec<SeriesView> {
    (1..=n)
        .map(|i| SeriesView {
            name: format!("row{i:02}"),
            value: Some(i as f64),
            points: vec![],
            visible: true,
        })
        .collect()
}

#[test]
fn content_fit_projector_respects_matching_and_fill() {
    let mut app = content_fit_app();
    for (width, height, matching, fill, want) in [
        (
            100,
            48,
            false,
            false,
            vec![
                (1, 4, 49, 7),
                (50, 4, 49, 24),
                (1, 28, 49, 17),
                (50, 28, 49, 3),
            ],
        ),
        (
            100,
            48,
            true,
            false,
            vec![
                (1, 4, 49, 24),
                (50, 4, 49, 24),
                (1, 28, 49, 17),
                (50, 28, 49, 17),
            ],
        ),
        (
            140,
            48,
            false,
            false,
            vec![
                (1, 4, 46, 7),
                (47, 4, 46, 24),
                (93, 4, 46, 18),
                (1, 28, 46, 3),
            ],
        ),
        (
            140,
            60,
            true,
            true,
            vec![
                (1, 4, 46, 30),
                (47, 4, 46, 30),
                (93, 4, 46, 30),
                (1, 34, 46, 23),
            ],
        ),
        (
            140,
            60,
            false,
            true,
            vec![
                (1, 4, 46, 7),
                (47, 4, 46, 24),
                (93, 4, 46, 18),
                (1, 34, 46, 3),
            ],
        ),
    ] {
        let DashboardLayoutItem::AutoGrid(group) = &mut app.layout.items[0] else {
            panic!()
        };
        group.options.match_row_heights = matching;
        group.options.fill_screen = fill;
        let area = Rect::new(0, 0, width, height);
        let actual = crate::ui::visible_dashboard_rects(area, &app)
            .iter()
            .map(|r| (r.rect.x, r.rect.y, r.rect.width, r.rect.height))
            .collect::<Vec<_>>();
        assert_eq!(
            actual, want,
            "{width}x{height} matching={matching} fill={fill}"
        );
        assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
        auto_grid_render(
            &mut app,
            Size::new(width, height),
            &format!("content-fit-mixed-{width}x{height}-{matching}-{fill}"),
            &["A", "B", "C", "D"],
        );
    }
    let mut app = content_fit_app();
    assert_eq!(
        crate::ui::hit_test(&app, Rect::new(0, 0, 100, 48), 51, 36),
        None
    );
    let buffer = auto_grid_render(
        &mut app,
        Size::new(100, 48),
        "content-fit-gap",
        &["A", "B", "C", "D"],
    );
    assert!(buffer_text(&buffer).contains("row20"));
}

#[tokio::test]
async fn content_fit_scroll_reveals_last_row() {
    let mut app = content_fit_app();
    app.panels.truncate(1);
    app.panels[0].series = content_rows(20);
    let DashboardLayoutItem::AutoGrid(group) = &mut app.layout.items[0] else {
        panic!()
    };
    group.items.truncate(1);
    let size = Size::new(100, 24);
    let buffer = auto_grid_render(&mut app, size, "content-fit-home", &["A"]);
    assert!(buffer_text(&buffer).contains("row01"));
    assert!(!buffer_text(&buffer).contains("row20"));
    handle_key(key(KeyCode::End), size, &mut app).await.unwrap();
    assert_eq!(app.vertical_scroll, 3);
    let buffer = auto_grid_render(&mut app, size, "content-fit-end", &["A"]);
    assert!(buffer_text(&buffer).contains("row20"));
    assert!(!buffer_text(&buffer).contains("row01"));
    handle_key(key(KeyCode::Home), size, &mut app)
        .await
        .unwrap();
    assert_eq!(app.vertical_scroll, 0);
    let buffer = auto_grid_render(&mut app, size, "content-fit-home-restored", &["A"]);
    assert!(buffer_text(&buffer).contains("row01"));
}

#[tokio::test]
async fn content_fit_nested_keyboard_buffers_follow_rows_and_tabs() {
    use crate::dashboard::autogrid::{AutoGridOptions, DashboardAutoGrid};
    let mut app = content_fit_app();
    app.panels.truncate(3);
    let row = RowId::new(0);
    let tabs = TabGroupId::new(0);
    let grid = |panels| {
        DashboardLayoutItem::AutoGrid(DashboardAutoGrid {
            options: AutoGridOptions {
                fit_content: true,
                min_height: Some(0),
                match_row_heights: false,
                ..Default::default()
            },
            items: crate::dashboard::autogrid::test_items(panels),
        })
    };
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
    let size = Size::new(140, 48);
    assert_eq!(app.visible_panel_indices(), vec![0, 1]);
    auto_grid_render(&mut app, size, "content-fit-nested-initial", &["A", "B"]);
    handle_key(key(KeyCode::Enter), size, &mut app)
        .await
        .unwrap();
    assert!(app.visible_panel_indices().is_empty());
    auto_grid_render(&mut app, size, "content-fit-nested-collapsed", &[]);
    handle_key(key(KeyCode::Enter), size, &mut app)
        .await
        .unwrap();
    assert_eq!(app.visible_panel_indices(), vec![0, 1]);
    auto_grid_render(&mut app, size, "content-fit-nested-expanded", &["A", "B"]);
    handle_key(key(KeyCode::Down), size, &mut app)
        .await
        .unwrap();
    assert_eq!(app.selected_item, Some(DashboardItemId::Tabs(tabs)));
    auto_grid_render(
        &mut app,
        size,
        "content-fit-nested-tabs-focused",
        &["A", "B"],
    );
    handle_key(key(KeyCode::Right), size, &mut app)
        .await
        .unwrap();
    assert_eq!(app.visible_panel_indices(), vec![2]);
    auto_grid_render(&mut app, size, "content-fit-nested-second", &["C"]);
    handle_key(key(KeyCode::Left), size, &mut app)
        .await
        .unwrap();
    assert_eq!(app.visible_panel_indices(), vec![0, 1]);
    auto_grid_render(&mut app, size, "content-fit-nested-first", &["A", "B"]);
}

#[tokio::test]
async fn content_fit_gap_click_errors_shrink_and_tiny_buffers() {
    let mut app = content_fit_app();
    let size = Size::new(100, 48);
    handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 51,
            row: 36,
            modifiers: KeyModifiers::NONE,
        },
        size,
        &mut app,
    )
    .await
    .unwrap();
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
    for error in [Some("first line\nsecond 水é".to_owned()), None] {
        app.panels[1].last_error = error.clone();
        let item = crate::ui::visible_dashboard_rects(Rect::new(0, 0, 100, 48), &app)
            .into_iter()
            .find(|r| r.id == DashboardItemId::Panel(1))
            .unwrap();
        assert_eq!(item.rect.height, if error.is_some() { 18 } else { 24 });
        auto_grid_render(
            &mut app,
            size,
            if error.is_some() {
                "content-fit-refresh-error"
            } else {
                "content-fit-refresh-recovered"
            },
            &["A", "B", "C", "D"],
        );
    }
    app.panels.truncate(1);
    app.panels[0].series = content_rows(1);
    let DashboardLayoutItem::AutoGrid(group) = &mut app.layout.items[0] else {
        panic!()
    };
    group.items.truncate(1);
    app.vertical_scroll = 70_000;
    crate::ui::clamp_dashboard_scroll(Rect::new(0, 0, 100, 24), &mut app);
    assert_eq!(app.vertical_scroll, 0);
    let buffer = auto_grid_render(
        &mut app,
        Size::new(100, 24),
        "content-fit-shrink-clamped",
        &["A"],
    );
    assert!(buffer_text(&buffer).contains("row01"));
    assert!(!buffer_text(&buffer).contains("row02"));
    for (w, h) in [(3, 8), (1, 1), (0, 0)] {
        crate::ui::clamp_dashboard_scroll(Rect::new(0, 0, w, h), &mut app);
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| crate::ui::draw_ui(f, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer.area, Rect::new(0, 0, w, h));
        assert!(!buffer_text(buffer).contains("row01"));
    }
}

#[tokio::test]
async fn content_fit_mouse_collapse_bounds_document_scroll() {
    let mut app = content_fit_app();
    let children = app.layout.items.clone();
    app.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::Row(
        DashboardRow::new(RowId::new(0), "R", false, false, children),
    )]));
    let size = Size::new(100, 24);
    // Header remains visible at zero while content is long; collapse changes the limit.
    handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        size,
        &mut app,
    )
    .await
    .unwrap();
    assert!(app.visible_panel_indices().is_empty());
    assert_eq!(app.vertical_scroll, 0);
    auto_grid_render(&mut app, size, "content-fit-mouse-collapsed", &[]);
    handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        size,
        &mut app,
    )
    .await
    .unwrap();
    assert_eq!(app.visible_panel_indices(), vec![0, 1, 2, 3]);
    assert_eq!(app.selected_item, Some(DashboardItemId::Row(RowId::new(0))));
    auto_grid_render(
        &mut app,
        size,
        "content-fit-mouse-expanded",
        &["A", "B", "C", "D"],
    );
}

#[tokio::test]
async fn content_fit_mouse_tab_switch_clamps_shorter_content() {
    use crate::dashboard::autogrid::{AutoGridOptions, DashboardAutoGrid, test_items};
    let mut app = content_fit_app();
    let grid = |indices| {
        DashboardLayoutItem::AutoGrid(DashboardAutoGrid {
            options: AutoGridOptions {
                fit_content: true,
                min_height: Some(0),
                match_row_heights: false,
                ..Default::default()
            },
            items: test_items(indices),
        })
    };
    app.apply_layout(DashboardLayout::new(vec![
        grid(vec![1]),
        DashboardLayoutItem::Tabs(DashboardTabs::new(
            TabGroupId::new(0),
            vec![
                DashboardTab {
                    title: "First".into(),
                    children: vec![grid(vec![1])],
                },
                DashboardTab {
                    title: "Second".into(),
                    children: vec![],
                },
            ],
        )),
    ]));
    app.vertical_scroll = 7;
    let size = Size::new(100, 24);
    let header = crate::ui::visible_dashboard_rects(Rect::new(0, 0, 100, 24), &app)
        .into_iter()
        .find(|r| matches!(r.kind, crate::ui::DashboardRectKind::Tabs { .. }))
        .unwrap();
    handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 13,
            row: header.rect.y,
            modifiers: KeyModifiers::NONE,
        },
        size,
        &mut app,
    )
    .await
    .unwrap();
    assert_eq!(app.layout.tabs(TabGroupId::new(0)).unwrap().active, Some(1));
    assert_eq!(app.vertical_scroll, 3);
    assert_eq!(
        app.selected_item,
        Some(DashboardItemId::Tabs(TabGroupId::new(0)))
    );
    auto_grid_render(&mut app, size, "content-fit-tab-shortened", &["B"]);
}

#[test]
fn content_fit_hidden_header_and_formatted_table_buffer() {
    use crate::app::{ThresholdMode, ThresholdStep, Thresholds};
    use ratatui::style::{Color, Modifier};
    let mut app = content_fit_app();
    app.panels.truncate(1);
    let DashboardLayoutItem::AutoGrid(group) = &mut app.layout.items[0] else {
        panic!()
    };
    group.items.truncate(1);
    app.panels[0].series[0].visible = false;
    app.panels[0].series[1].value = None;
    app.panels[0].display.no_value = Some("missing".into());
    app.panels[0].thresholds = Some(Thresholds {
        mode: ThresholdMode::Absolute,
        steps: vec![ThresholdStep {
            value: None,
            color: Color::Red,
        }],
        style: None,
    });
    let children = app.layout.items.clone();
    app.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::Row(
        DashboardRow::new(RowId::new(0), "Hidden", true, true, children),
    )]));
    assert_eq!(app.visible_panel_indices(), vec![0]);
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
    let buffer = auto_grid_render(
        &mut app,
        Size::new(100, 24),
        "content-fit-hidden-formatted",
        &["A"],
    );
    let text = buffer_text(&buffer);
    assert!(!text.contains("Hidden"));
    assert!(!text.contains("row01"));
    assert!(text.contains("row02"));
    assert!(text.contains("missing"));
    assert!(text.contains("row03"));
    assert_eq!(buffer.cell((2, 5)).unwrap().fg, app.theme.title);
    assert!(
        buffer
            .cell((2, 5))
            .unwrap()
            .modifier
            .contains(Modifier::BOLD)
    );
    assert!(
        buffer
            .content
            .iter()
            .any(|c| c.symbol() == "3" && c.fg == Color::Red)
    );
    for width in [3, 6, 10, 40] {
        assert_eq!(measure_panel_content(&app.panels[0], width), Some(6));
    }
}
