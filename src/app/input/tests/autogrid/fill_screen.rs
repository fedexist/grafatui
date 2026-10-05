use super::*;
use crate::dashboard::autogrid::{AutoGridOptions, DashboardAutoGrid};

fn filled_grid(panels: Vec<usize>) -> DashboardLayoutItem {
    DashboardLayoutItem::AutoGrid(DashboardAutoGrid {
        options: AutoGridOptions {
            fill_screen: true,
            ..Default::default()
        },
        panels,
    })
}
fn filled_app() -> AppState {
    let mut app = auto_grid_test_app();
    app.apply_layout(DashboardLayout::new(vec![filled_grid(vec![0, 1, 2, 3])]));
    app
}
fn render(app: &mut AppState, size: Size, label: &str, titles: &[&str]) {
    app.view_end_ts = 1_783_080_000;
    let buffer = auto_grid_render(app, size, label, titles);
    for item in crate::ui::visible_dashboard_rects(Rect::new(0, 0, size.width, size.height), app) {
        match item.kind {
            crate::ui::DashboardRectKind::Row { collapsed, .. } => {
                let cell = buffer.cell((item.rect.x, item.rect.y)).unwrap();
                assert_eq!(cell.symbol(), if collapsed { "▶" } else { "▼" });
                if app.selected_item == Some(item.id) {
                    assert_eq!(cell.fg, app.theme.border_selected);
                    assert!(cell.modifier.contains(ratatui::style::Modifier::BOLD));
                }
            }
            crate::ui::DashboardRectKind::Tabs { .. } => {
                let cells = (item.rect.x..item.rect.right())
                    .map(|x| buffer.cell((x, item.rect.y)).unwrap())
                    .collect::<Vec<_>>();
                assert!(
                    cells
                        .iter()
                        .any(|c| c.modifier.contains(ratatui::style::Modifier::BOLD))
                );
                if app.selected_item == Some(item.id) {
                    assert!(
                        cells
                            .iter()
                            .any(|c| c.modifier.contains(ratatui::style::Modifier::UNDERLINED))
                    );
                }
            }
            _ => {}
        }
    }
}

#[test]
fn fill_screen_actual_buffers_stretch_on_width_and_height_changes() {
    let mut app = filled_app();
    for (w, h, rects) in [
        (
            140,
            48,
            vec![
                (1, 4, 46, 21),
                (47, 4, 46, 21),
                (93, 4, 46, 21),
                (1, 25, 46, 20),
            ],
        ),
        (
            100,
            48,
            vec![
                (1, 4, 49, 21),
                (50, 4, 49, 21),
                (1, 25, 49, 20),
                (50, 25, 49, 20),
            ],
        ),
        (
            140,
            60,
            vec![
                (1, 4, 46, 27),
                (47, 4, 46, 27),
                (93, 4, 46, 27),
                (1, 31, 46, 26),
            ],
        ),
    ] {
        assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
        let area = Rect::new(0, 0, w, h);
        crate::ui::scroll_selected_into_view(area, &mut app);
        assert_eq!(
            crate::ui::visible_dashboard_rects(area, &app)
                .iter()
                .map(|r| (r.rect.x, r.rect.y, r.rect.width, r.rect.height))
                .collect::<Vec<_>>(),
            rects
        );
        render(
            &mut app,
            Size::new(w, h),
            &format!("stretch-{w}x{h}"),
            &["A", "B", "C", "D"],
        );
    }
}

#[tokio::test]
async fn fill_screen_input_resize_and_click_preserve_focus() {
    let mut app = filled_app();
    app.selected_item = Some(DashboardItemId::Panel(3));
    for (w, h, scroll, titles) in [
        (140, 48, 0, vec!["A", "B", "C", "D"]),
        (40, 24, 18, vec!["D"]),
        (100, 48, 0, vec!["A", "B", "C", "D"]),
    ] {
        crate::ui::scroll_selected_into_view(Rect::new(0, 0, w, h), &mut app);
        assert_eq!(app.vertical_scroll, scroll);
        assert_eq!(app.selected_item, Some(DashboardItemId::Panel(3)));
        render(
            &mut app,
            Size::new(w, h),
            &format!("resize-focus-{w}x{h}"),
            &titles,
        );
    }
    handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 50,
            row: 25,
            modifiers: KeyModifiers::NONE,
        },
        Size::new(100, 48),
        &mut app,
    )
    .await
    .unwrap();
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(3)));
    render(
        &mut app,
        Size::new(100, 48),
        "resize-focus-click",
        &["A", "B", "C", "D"],
    );
}

#[tokio::test]
async fn fill_screen_nested_keyboard_buffers_follow_rows_and_tabs() {
    let mut app = filled_app();
    app.panels.truncate(3);
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
                        children: vec![filled_grid(vec![0, 1])],
                    },
                    DashboardTab {
                        title: "Second".into(),
                        children: vec![filled_grid(vec![2])],
                    },
                ],
            ))],
        ),
    )]));
    let size = Size::new(140, 48);
    assert_eq!(app.selected_item, Some(DashboardItemId::Row(row)));
    render(&mut app, size, "nested-initial", &["A", "B"]);
    for (keycode, label, selected, titles) in [
        (
            KeyCode::Enter,
            "nested-collapsed",
            DashboardItemId::Row(row),
            vec![],
        ),
        (
            KeyCode::Enter,
            "nested-expanded",
            DashboardItemId::Row(row),
            vec!["A", "B"],
        ),
        (
            KeyCode::Down,
            "nested-tabs-focused",
            DashboardItemId::Tabs(tabs),
            vec!["A", "B"],
        ),
        (
            KeyCode::Right,
            "nested-second",
            DashboardItemId::Tabs(tabs),
            vec!["C"],
        ),
        (
            KeyCode::Left,
            "nested-first",
            DashboardItemId::Tabs(tabs),
            vec!["A", "B"],
        ),
    ] {
        handle_key(key(keycode), size, &mut app).await.unwrap();
        assert_eq!(app.selected_item, Some(selected));
        assert_eq!(app.visible_panel_indices().len(), titles.len());
        if keycode == KeyCode::Enter && !titles.is_empty() {
            assert!(app.panels.iter().take(2).all(|p| p.series.is_empty()));
        }
        render(&mut app, size, label, &titles);
    }
}

#[tokio::test]
async fn fill_screen_following_sibling_is_reachable_with_end_and_home() {
    let mut app = filled_app();
    app.apply_layout(DashboardLayout::new(vec![
        filled_grid(vec![0, 1, 2, 3]),
        DashboardLayoutItem::Row(DashboardRow::new(
            RowId::new(0),
            "After",
            true,
            false,
            vec![],
        )),
    ]));
    let size = Size::new(140, 48);
    render(&mut app, size, "siblings-initial", &["A", "B", "C", "D"]);
    handle_key(key(KeyCode::End), size, &mut app).await.unwrap();
    assert_eq!(app.vertical_scroll, 1);
    let rects = crate::ui::visible_dashboard_rects(Rect::new(0, 0, 140, 48), &app);
    assert_eq!(
        rects.last().unwrap().id,
        DashboardItemId::Row(RowId::new(0))
    );
    assert_eq!(rects.last().unwrap().rect, Rect::new(1, 42, 138, 1));
    render(&mut app, size, "siblings-end", &["A", "B", "C", "D"]);
    handle_key(key(KeyCode::Home), size, &mut app)
        .await
        .unwrap();
    assert_eq!(app.vertical_scroll, 0);
    render(&mut app, size, "siblings-home", &["A", "B", "C", "D"]);
}

#[test]
fn fill_screen_empty_group_has_no_phantom_height_or_header() {
    let mut app = filled_app();
    let size = Size::new(40, 12);
    app.apply_layout(DashboardLayout::new(vec![filled_grid(vec![])]));
    assert_eq!(app.selected_item, None);
    render(&mut app, size, "empty-root", &[]);
    app.apply_layout(DashboardLayout::new(vec![
        filled_grid(vec![]),
        filled_grid(vec![0]),
    ]));
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
    assert_eq!(
        crate::ui::visible_dashboard_rects(Rect::new(0, 0, 40, 12), &app)[0].rect,
        Rect::new(1, 4, 38, 5)
    );
    render(&mut app, size, "empty-sibling", &["A"]);
}

#[tokio::test]
async fn fill_screen_terminal_flow_buffers_keep_identity() {
    let mut app = filled_app();
    let wide = Size::new(100, 48);
    render(&mut app, wide, "lifecycle-initial", &["A", "B", "C", "D"]);
    for index in 1..4 {
        handle_key(key(KeyCode::Down), wide, &mut app)
            .await
            .unwrap();
        assert_eq!(app.selected_item, Some(DashboardItemId::Panel(index)));
        render(
            &mut app,
            wide,
            &format!("lifecycle-down-{index}"),
            &["A", "B", "C", "D"],
        );
    }
    crate::ui::scroll_selected_into_view(Rect::new(0, 0, 40, 24), &mut app);
    assert_eq!(app.vertical_scroll, 18);
    render(&mut app, Size::new(40, 24), "lifecycle-shrink", &["D"]);
    crate::ui::scroll_selected_into_view(Rect::new(0, 0, 100, 48), &mut app);
    assert_eq!(app.vertical_scroll, 0);
    render(&mut app, wide, "lifecycle-grow", &["A", "B", "C", "D"]);
    handle_key(key(KeyCode::End), wide, &mut app).await.unwrap();
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(3)));
    assert_eq!(app.vertical_scroll, 0);
    render(&mut app, wide, "lifecycle-end", &["A", "B", "C", "D"]);
    handle_key(key(KeyCode::Up), wide, &mut app).await.unwrap();
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(2)));
    render(&mut app, wide, "lifecycle-up", &["A", "B", "C", "D"]);
}
