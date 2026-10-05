use super::*;

fn auto_grid_test_app() -> AppState {
    use crate::dashboard::autogrid::{AutoGridOptions, DashboardAutoGrid};
    let mut app = test_app();
    app.title = "AutoGrid".into();
    app.view_end_ts = 1_783_080_000;
    app.panels = ["A", "B", "C", "D"]
        .into_iter()
        .enumerate()
        .map(|(index, title)| {
            let mut panel = test_panel(title);
            panel.panel_type = PanelType::Stat;
            panel.series = vec![SeriesView {
                name: title.into(),
                value: Some((index + 1) as f64),
                points: vec![],
                visible: true,
            }];
            panel
        })
        .collect();
    app.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::AutoGrid(
        DashboardAutoGrid {
            options: AutoGridOptions::default(),
            panels: vec![0, 1, 2, 3],
        },
    )]));
    app
}

fn auto_grid_render(
    app: &mut AppState,
    size: Size,
    label: &str,
    expected_titles: &[&str],
) -> ratatui::buffer::Buffer {
    use ratatui::{Terminal, backend::TestBackend};
    let mut terminal = Terminal::new(TestBackend::new(size.width, size.height)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw_ui(frame, app))
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let text = (0..size.height)
        .map(|y| {
            (0..size.width)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    for title in ["A", "B", "C", "D"] {
        assert_eq!(
            text.contains(&format!("┌{title}")),
            expected_titles.contains(&title),
            "{label}: unexpected visible title {title}"
        );
    }
    let area = Rect::new(0, 0, size.width, size.height);
    for item in crate::ui::visible_dashboard_rects(area, app) {
        if let crate::ui::DashboardRectKind::Panel { index } = item.kind {
            assert_eq!(
                buffer
                    .cell((item.rect.x + 1, item.rect.y))
                    .unwrap()
                    .symbol(),
                app.panels[index].title,
                "{label}: panel title missing"
            );
            if app.selected_item == Some(item.id) {
                assert_eq!(
                    buffer.cell((item.rect.x, item.rect.y)).unwrap().fg,
                    app.theme.border_selected,
                    "{label}: focus border missing"
                );
            }
        }
    }
    buffer
}

#[test]
fn auto_grid_actual_buffers_reflow_on_resize() {
    let mut app = auto_grid_test_app();
    for (width, height, expected) in [
        (140, 40, vec![0, 1, 2, 3]),
        (100, 40, vec![0, 1, 2, 3]),
        (40, 24, vec![0]),
    ] {
        let size = Size::new(width, height);
        let area = Rect::new(0, 0, width, height);
        crate::ui::scroll_selected_into_view(area, &mut app);
        assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
        assert_eq!(
            crate::ui::visible_dashboard_rects(area, &app)
                .iter()
                .filter_map(|r| match r.kind {
                    crate::ui::DashboardRectKind::Panel { index } => Some(index),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            expected
        );
        let titles: &[&str] = if width == 40 {
            &["A"]
        } else {
            &["A", "B", "C", "D"]
        };
        auto_grid_render(&mut app, size, &format!("columns-{width}x{height}"), titles);
    }
}

#[tokio::test]
async fn auto_grid_keyboard_resize_and_mouse_preserve_focus() {
    let mut app = auto_grid_test_app();
    let short = Size::new(100, 12);
    auto_grid_render(&mut app, short, "scroll-initial", &["A", "B"]);
    for _ in 0..3 {
        handle_key(key(KeyCode::Down), short, &mut app)
            .await
            .unwrap();
    }
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(3)));
    assert!(app.vertical_scroll > 0);
    auto_grid_render(&mut app, short, "scroll-selected", &["C", "D"]);
    let narrow = Size::new(40, 12);
    let area = Rect::new(0, 0, narrow.width, narrow.height);
    crate::ui::scroll_selected_into_view(area, &mut app);
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(3)));
    auto_grid_render(&mut app, narrow, "scroll-resized", &["D"]);
    let selected = crate::ui::visible_dashboard_rects(area, &app)
        .into_iter()
        .find(|item| item.id == DashboardItemId::Panel(3))
        .unwrap();
    handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: selected.rect.x,
            row: selected.rect.y,
            modifiers: KeyModifiers::NONE,
        },
        narrow,
        &mut app,
    )
    .await
    .unwrap();
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(3)));
    auto_grid_render(&mut app, narrow, "scroll-clicked", &["D"]);
}

#[tokio::test]
async fn auto_grid_nested_keyboard_buffers_follow_rows_and_tabs() {
    use crate::dashboard::autogrid::{AutoGridOptions, DashboardAutoGrid};
    let mut app = auto_grid_test_app();
    app.panels.truncate(3);
    let row = RowId::new(0);
    let tabs = TabGroupId::new(0);
    let grid = |panels| {
        DashboardLayoutItem::AutoGrid(DashboardAutoGrid {
            options: AutoGridOptions::default(),
            panels,
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
    let size = Size::new(140, 40);
    assert_eq!(app.visible_panel_indices(), vec![0, 1]);
    auto_grid_render(&mut app, size, "nested-initial", &["A", "B"]);
    handle_key(key(KeyCode::Enter), size, &mut app)
        .await
        .unwrap();
    assert!(app.visible_panel_indices().is_empty());
    auto_grid_render(&mut app, size, "nested-collapsed", &[]);
    handle_key(key(KeyCode::Enter), size, &mut app)
        .await
        .unwrap();
    assert_eq!(app.visible_panel_indices(), vec![0, 1]);
    auto_grid_render(&mut app, size, "nested-expanded", &["A", "B"]);
    handle_key(key(KeyCode::Down), size, &mut app)
        .await
        .unwrap();
    assert_eq!(app.selected_item, Some(DashboardItemId::Tabs(tabs)));
    auto_grid_render(&mut app, size, "nested-tabs-focused", &["A", "B"]);
    handle_key(key(KeyCode::Right), size, &mut app)
        .await
        .unwrap();
    assert_eq!(app.visible_panel_indices(), vec![2]);
    auto_grid_render(&mut app, size, "nested-second", &["C"]);
    handle_key(key(KeyCode::Left), size, &mut app)
        .await
        .unwrap();
    assert_eq!(app.visible_panel_indices(), vec![0, 1]);
    auto_grid_render(&mut app, size, "nested-first", &["A", "B"]);
}

#[test]
fn auto_grid_empty_actual_buffer_has_no_phantom_group() {
    use crate::dashboard::autogrid::{AutoGridOptions, DashboardAutoGrid};
    let mut app = auto_grid_test_app();
    let empty = DashboardLayoutItem::AutoGrid(DashboardAutoGrid {
        options: AutoGridOptions::default(),
        panels: vec![],
    });
    app.apply_layout(DashboardLayout::new(vec![empty.clone()]));
    assert_eq!(app.selected_item, None);
    auto_grid_render(&mut app, Size::new(40, 12), "empty-root", &[]);
    app.apply_layout(DashboardLayout::new(vec![
        empty,
        DashboardLayoutItem::Panel(0),
    ]));
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
    auto_grid_render(&mut app, Size::new(40, 12), "empty-sibling", &["A"]);
}

#[tokio::test]
async fn auto_grid_terminal_flow_buffers_keep_identity() {
    let mut app = auto_grid_test_app();
    let initial = Size::new(100, 24);
    auto_grid_render(&mut app, initial, "lifecycle-initial", &["A", "B"]);
    handle_key(key(KeyCode::Down), initial, &mut app)
        .await
        .unwrap();
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(1)));
    auto_grid_render(&mut app, initial, "lifecycle-down", &["A", "B"]);
    let resized = Size::new(40, 24);
    crate::ui::scroll_selected_into_view(Rect::new(0, 0, 40, 24), &mut app);
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(1)));
    auto_grid_render(&mut app, resized, "lifecycle-resized", &["B"]);
    handle_key(key(KeyCode::Up), resized, &mut app)
        .await
        .unwrap();
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
    auto_grid_render(&mut app, resized, "lifecycle-up", &["A"]);
}

#[tokio::test]
async fn auto_grid_end_and_manual_scroll_stay_within_document() {
    let mut app = auto_grid_test_app();
    let size = Size::new(100, 24);
    auto_grid_render(&mut app, size, "manual-initial", &["A", "B"]);
    handle_key(key(KeyCode::End), size, &mut app).await.unwrap();
    assert_eq!(app.vertical_scroll, 7);
    auto_grid_render(&mut app, size, "manual-end", &["C", "D"]);
    handle_key(key(KeyCode::PageUp), size, &mut app)
        .await
        .unwrap();
    assert_eq!(app.vertical_scroll, 0);
    auto_grid_render(&mut app, size, "manual-page-up", &["A", "B"]);
    for _ in 0..3 {
        handle_key(key(KeyCode::PageDown), size, &mut app)
            .await
            .unwrap();
        assert_eq!(app.vertical_scroll, 7);
    }
    assert_eq!(app.vertical_scroll, 7);
    auto_grid_render(&mut app, size, "manual-page-down", &["C", "D"]);
    handle_mouse(
        MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 1,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        size,
        &mut app,
    )
    .await
    .unwrap();
    assert_eq!(app.vertical_scroll, 6);
    auto_grid_render(&mut app, size, "manual-wheel-up", &["C", "D"]);
    handle_key(key(KeyCode::Home), size, &mut app)
        .await
        .unwrap();
    assert_eq!(app.vertical_scroll, 0);
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
    auto_grid_render(&mut app, size, "manual-home", &["A", "B"]);
}
