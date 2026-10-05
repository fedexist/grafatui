use super::*;

#[test]
fn auto_grid_solver_reflows_at_minimum_width_breakpoints() {
    use super::super::autogrid::project_auto_grid;
    use crate::dashboard::autogrid::AutoGridOptions;
    let options = AutoGridOptions::default();
    for (width, columns) in [(44, 1), (45, 1), (89, 1), (90, 2), (134, 2), (135, 3)] {
        let projected = project_auto_grid(5, width, 7, 0, &options, &[8, 3, 6, 2]);
        assert_eq!(projected.panels[columns].rect.y, 25, "width={width}");
        assert_eq!(
            projected.panels.iter().map(|p| p.index).collect::<Vec<_>>(),
            vec![8, 3, 6, 2]
        );
        assert_eq!(projected.panels[0].rect.x, 5);
        assert_eq!(projected.content_height, if columns == 1 { 72 } else { 36 });
    }
    let projected = project_auto_grid(0, 136, 0, 0, &options, &[0, 1, 2, 3]);
    assert_eq!(
        projected
            .panels
            .iter()
            .take(3)
            .map(|p| (p.rect.x, p.rect.width))
            .collect::<Vec<_>>(),
        vec![(0, 46), (46, 45), (91, 45)]
    );
    assert_eq!(projected.panels[3].rect.y, 18);
}

#[test]
fn auto_grid_solver_handles_empty_small_and_large_documents() {
    use super::super::autogrid::project_auto_grid;
    use crate::dashboard::autogrid::AutoGridOptions;
    let options = AutoGridOptions {
        max_columns: 3,
        min_column_width: 20,
        row_height: 70_000,
        fill_screen: false,
        ..Default::default()
    };
    assert_eq!(
        project_auto_grid(0, 0, 0, 0, &options, &[0]).content_height,
        0
    );
    assert!(
        project_auto_grid(0, 40, 0, 0, &options, &[])
            .panels
            .is_empty()
    );
    let small = project_auto_grid(0, 1, 0, 0, &options, &[0, 1]);
    assert_eq!(small.panels[0].rect.width, 1);
    assert_eq!(small.panels[1].rect.y, 70_000);
    let large = project_auto_grid(2, 19, 70_000, 0, &options, &[0, 1, 2]);
    assert_eq!(
        large.panels.iter().map(|p| p.rect.y).collect::<Vec<_>>(),
        vec![70_000, 140_000, 210_000]
    );
    assert_eq!(large.content_height, 210_000);
    let capped = project_auto_grid(
        0,
        200,
        0,
        0,
        &AutoGridOptions {
            max_columns: 2,
            min_column_width: 20,
            row_height: 10,
            fill_screen: false,
            ..Default::default()
        },
        &[0, 1, 2],
    );
    assert_eq!(capped.panels[2].rect.y, 10);
    let fewer = project_auto_grid(0, 200, 0, 0, &options, &[0]);
    assert_eq!(fewer.panels[0].rect.width, 200);
}

fn auto_grid_app(options: crate::dashboard::autogrid::AutoGridOptions) -> AppState {
    app_with(
        ["A", "B", "C", "D"]
            .into_iter()
            .map(|title| panel(title, None))
            .collect(),
        DashboardLayout::new(vec![DashboardLayoutItem::AutoGrid(
            crate::dashboard::autogrid::DashboardAutoGrid {
                options,
                items: crate::dashboard::autogrid::test_items(vec![0, 1, 2, 3]),
            },
        )]),
    )
}

#[test]
fn auto_grid_large_document_scrolls_without_overlap() {
    let mut app = auto_grid_app(crate::dashboard::autogrid::AutoGridOptions {
        max_columns: 1,
        min_column_width: 20,
        row_height: 70_002,
        fill_screen: false,
        ..Default::default()
    });
    app.selected_item = Some(DashboardItemId::Panel(1));
    scroll_selected_into_view(Rect::new(0, 0, 100, 24), &mut app);
    assert!(app.vertical_scroll > 20_000);
    let rects = visible_dashboard_rects(Rect::new(0, 0, 100, 24), &app);
    assert_eq!(rects.len(), 1);
    assert_eq!(rects[0].id, DashboardItemId::Panel(1));
    assert_eq!(rects[0].rect, Rect::new(1, 4, 98, 17));
    assert_eq!(
        hit_test(&app, Rect::new(0, 0, 100, 24), 1, 4).unwrap().id,
        DashboardItemId::Panel(1)
    );
}

#[test]
fn auto_grid_resize_retains_selected_panel() {
    let mut app = auto_grid_app(crate::dashboard::autogrid::AutoGridOptions::default());
    app.selected_item = Some(DashboardItemId::Panel(3));
    for (width, expected_y, expected_x) in [(140, 18, 1), (100, 18, 50), (40, 54, 1)] {
        let viewport = Rect::new(0, 0, width, 80);
        scroll_selected_into_view(viewport, &mut app);
        assert_eq!(app.selected_item, Some(DashboardItemId::Panel(3)));
        let inner = dashboard_inner_area(viewport);
        let rects = visible_dashboard_rects(viewport, &app);
        let selected = rects
            .iter()
            .find(|r| r.id == DashboardItemId::Panel(3))
            .unwrap();
        assert_eq!(selected.rect.y, inner.y + expected_y);
        assert_eq!(selected.rect.x, expected_x);
        assert_eq!(
            hit_test(&app, viewport, selected.rect.x, selected.rect.y)
                .unwrap()
                .id,
            selected.id
        );
    }
}

#[test]
fn auto_grid_nested_projection_respects_active_and_collapsed_branches() {
    use crate::dashboard::autogrid::{AutoGridOptions, DashboardAutoGrid};
    let row = RowId::new(0);
    let tabs = TabGroupId::new(0);
    let mut app = app_with(
        ["A", "B", "C"]
            .into_iter()
            .map(|title| panel(title, None))
            .collect(),
        DashboardLayout::new(vec![DashboardLayoutItem::Row(DashboardRow::new(
            row,
            "R",
            false,
            false,
            vec![DashboardLayoutItem::Tabs(DashboardTabs::new(
                tabs,
                vec![
                    DashboardTab {
                        title: "First".into(),
                        children: vec![DashboardLayoutItem::AutoGrid(DashboardAutoGrid {
                            options: AutoGridOptions::default(),
                            items: crate::dashboard::autogrid::test_items(vec![0, 1]),
                        })],
                    },
                    DashboardTab {
                        title: "Second".into(),
                        children: vec![DashboardLayoutItem::AutoGrid(DashboardAutoGrid {
                            options: AutoGridOptions::default(),
                            items: crate::dashboard::autogrid::test_items(vec![2]),
                        })],
                    },
                ],
            ))],
        ))]),
    );
    let viewport = Rect::new(0, 0, 140, 40);
    let visible = visible_dashboard_rects(viewport, &app);
    assert_eq!(
        visible.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![
            DashboardItemId::Row(row),
            DashboardItemId::Tabs(tabs),
            DashboardItemId::Panel(0),
            DashboardItemId::Panel(1),
        ]
    );
    assert_eq!(visible[2].rect.y, visible[1].rect.y + 1);
    app.layout.set_row_collapsed(row, true).unwrap();
    assert_eq!(visible_dashboard_rects(viewport, &app).len(), 1);
    app.layout.set_row_collapsed(row, false).unwrap();
    app.layout.set_active_tab(tabs, 1).unwrap();
    assert_eq!(
        visible_panel_rects(viewport, &app)
            .iter()
            .map(|(_, i)| *i)
            .collect::<Vec<_>>(),
        vec![2]
    );
    app.layout.set_active_tab(tabs, 0).unwrap();
    assert_eq!(
        visible_panel_rects(viewport, &app)
            .iter()
            .map(|(_, i)| *i)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
}

#[test]
fn auto_grid_empty_group_does_not_move_following_sibling() {
    use crate::dashboard::autogrid::{AutoGridOptions, DashboardAutoGrid};
    let app = app_with(
        vec![panel("A", None)],
        DashboardLayout::new(vec![
            DashboardLayoutItem::AutoGrid(DashboardAutoGrid {
                options: AutoGridOptions::default(),
                items: crate::dashboard::autogrid::test_items(vec![]),
            }),
            DashboardLayoutItem::Panel(0),
        ]),
    );
    let viewport = Rect::new(0, 0, 40, 24);
    let rects = visible_dashboard_rects(viewport, &app);
    assert_eq!(rects.len(), 1);
    assert_eq!(rects[0].rect.y, dashboard_inner_area(viewport).y);
}

#[test]
fn auto_grid_sibling_after_large_extent_remains_reachable() {
    let mut app = auto_grid_app(crate::dashboard::autogrid::AutoGridOptions {
        max_columns: 1,
        min_column_width: 45,
        row_height: 70_000,
        fill_screen: false,
        ..Default::default()
    });
    app.layout
        .items
        .push(DashboardLayoutItem::Row(DashboardRow::new(
            RowId::new(0),
            "After",
            false,
            false,
            vec![],
        )));
    app.selected_item = Some(DashboardItemId::Row(RowId::new(0)));
    let viewport = Rect::new(0, 0, 100, 24);
    scroll_selected_into_view(viewport, &mut app);
    let rects = visible_dashboard_rects(viewport, &app);
    let header = rects
        .iter()
        .find(|r| r.id == DashboardItemId::Row(RowId::new(0)))
        .unwrap();
    assert_eq!(header.rect.height, 1);
    assert!(header.disclosure_rect.is_some());
}
