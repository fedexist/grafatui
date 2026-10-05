use super::super::autogrid::project_auto_grid;
use super::*;
use crate::dashboard::autogrid::AutoGridOptions;

#[test]
fn fill_screen_solver_grows_tracks_with_remainders_and_preserves_minima() {
    let options = AutoGridOptions {
        fill_screen: true,
        ..Default::default()
    };
    for (width, height, extent, tracks) in [
        (135, 41, 41, vec![(7, 21), (7, 21), (7, 21), (28, 20)]),
        (90, 41, 41, vec![(7, 21), (7, 21), (28, 20), (28, 20)]),
        (44, 41, 72, vec![(7, 18), (25, 18), (43, 18), (61, 18)]),
        (135, 35, 36, vec![(7, 18), (7, 18), (7, 18), (25, 18)]),
        (135, 36, 36, vec![(7, 18), (7, 18), (7, 18), (25, 18)]),
        (135, 0, 36, vec![(7, 18), (7, 18), (7, 18), (25, 18)]),
        (135, 40, 40, vec![(7, 20), (7, 20), (7, 20), (27, 20)]),
    ] {
        let p = project_auto_grid(5, width, 7, height, &options, &[8, 3, 6, 2]);
        assert_eq!(p.content_height, extent, "{width}x{height}");
        assert_eq!(
            p.panels
                .iter()
                .map(|p| (p.rect.y, p.rect.height))
                .collect::<Vec<_>>(),
            tracks
        );
        assert_eq!(
            p.panels.iter().map(|p| p.index).collect::<Vec<_>>(),
            vec![8, 3, 6, 2]
        );
    }
    let fixed = AutoGridOptions {
        fill_screen: false,
        ..options
    };
    assert_eq!(
        project_auto_grid(0, 135, 0, 100, &fixed, &[0, 1, 2, 3]).content_height,
        36
    );
    assert_eq!(
        project_auto_grid(0, 135, 0, 41, &options, &[0]).panels[0]
            .rect
            .height,
        41
    );
}

#[test]
fn fill_screen_solver_handles_empty_presets_and_wide_document_coordinates() {
    let options = AutoGridOptions {
        fill_screen: true,
        ..Default::default()
    };
    assert_eq!(
        project_auto_grid(0, 0, 0, 41, &options, &[0]).content_height,
        0
    );
    assert_eq!(
        project_auto_grid(0, 135, 0, 41, &options, &[]).content_height,
        0
    );
    for (baseline, extent, tracks) in [
        (10, 41, vec![21, 21, 21, 20]),
        (18, 41, vec![21, 21, 21, 20]),
        (29, 58, vec![29, 29, 29, 29]),
    ] {
        let o = AutoGridOptions {
            row_height: baseline,
            ..options
        };
        let p = project_auto_grid(0, 135, 0, 41, &o, &[0, 1, 2, 3]);
        assert_eq!(p.content_height, extent);
        assert_eq!(
            p.panels.iter().map(|p| p.rect.height).collect::<Vec<_>>(),
            tracks
        );
    }
    let huge = AutoGridOptions {
        row_height: 70_000,
        ..options
    };
    let p = project_auto_grid(0, 44, 70_000, u16::MAX, &huge, &[0, 1, 2]);
    assert_eq!(p.content_height, 210_000);
    assert_eq!(
        p.panels.iter().map(|p| p.rect.y).collect::<Vec<_>>(),
        vec![70_000, 140_000, 210_000]
    );
    assert_eq!(p.panels.len(), 3);
}

fn filled_grid(panels: Vec<usize>) -> DashboardLayoutItem {
    DashboardLayoutItem::AutoGrid(crate::dashboard::autogrid::DashboardAutoGrid {
        options: AutoGridOptions {
            fill_screen: true,
            ..Default::default()
        },
        items: crate::dashboard::autogrid::test_items(panels),
    })
}

#[test]
fn fill_screen_nested_headers_reduce_allocation_once() {
    let row = RowId::new(0);
    let tabs = TabGroupId::new(0);
    for hidden in [false, true] {
        let mut app = app_with(
            ["A", "B", "C"]
                .into_iter()
                .map(|title| panel(title, None))
                .collect(),
            DashboardLayout::new(vec![DashboardLayoutItem::Row(DashboardRow::new(
                row,
                "R",
                false,
                hidden,
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
            ))]),
        );
        let viewport = Rect::new(0, 0, 140, 48);
        let (logical, _) = projected_dashboard_rects(dashboard_inner_area(viewport), &app);
        let panel_height = logical
            .iter()
            .find(|p| p.id == DashboardItemId::Panel(0))
            .unwrap()
            .rect
            .height;
        assert_eq!(panel_height, if hidden { 40 } else { 39 });
        let rects = visible_panel_rects(viewport, &app);
        assert_eq!(
            rects,
            if hidden {
                vec![(Rect::new(1, 5, 69, 40), 0), (Rect::new(70, 5, 69, 40), 1)]
            } else {
                vec![(Rect::new(1, 6, 69, 39), 0), (Rect::new(70, 6, 69, 39), 1)]
            }
        );
        app.layout.set_active_tab(tabs, 1).unwrap();
        assert_eq!(
            visible_panel_rects(viewport, &app),
            vec![(
                if hidden {
                    Rect::new(1, 5, 138, 40)
                } else {
                    Rect::new(1, 6, 138, 39)
                },
                2
            )]
        );
        if !hidden {
            app.layout.set_row_collapsed(row, true).unwrap();
            assert!(visible_panel_rects(viewport, &app).is_empty());
        }
    }
    // Repeated nesting must deduct each header, independent of document position.
    let mut child = filled_grid(vec![0]);
    for i in 0..4 {
        child = DashboardLayoutItem::Row(DashboardRow::new(
            RowId::new(i),
            "R",
            false,
            false,
            vec![child],
        ));
    }
    let app = app_with(vec![panel("A", None)], DashboardLayout::new(vec![child]));
    assert_eq!(
        visible_panel_rects(Rect::new(0, 0, 140, 48), &app),
        vec![(Rect::new(1, 8, 138, 37), 0)]
    );
}

#[test]
fn fill_screen_resize_reconciles_stored_scroll() {
    let mut app = app_with(
        ["A", "B", "C", "D"]
            .into_iter()
            .map(|title| panel(title, None))
            .collect(),
        DashboardLayout::new(vec![filled_grid(vec![0, 1, 2, 3])]),
    );
    app.selected_item = Some(DashboardItemId::Panel(3));
    for (viewport, scroll, want) in [
        (Rect::new(0, 0, 140, 48), 0, Rect::new(1, 25, 46, 20)),
        (Rect::new(0, 0, 40, 24), 18, Rect::new(1, 4, 38, 17)),
        (Rect::new(0, 0, 100, 48), 0, Rect::new(50, 25, 49, 20)),
    ] {
        scroll_selected_into_view(viewport, &mut app);
        assert_eq!(app.vertical_scroll, scroll);
        assert_eq!(app.selected_item, Some(DashboardItemId::Panel(3)));
        let rect = visible_panel_rects(viewport, &app)
            .into_iter()
            .find(|(_, i)| *i == 3)
            .unwrap()
            .0;
        assert_eq!(rect, want);
        assert_eq!(
            hit_test(&app, viewport, rect.x, rect.y).unwrap().id,
            DashboardItemId::Panel(3)
        );
    }
}

#[test]
fn fill_screen_sibling_allocations_ignore_document_position_and_scroll() {
    let mut app = app_with(
        ["A", "B"]
            .into_iter()
            .map(|title| panel(title, None))
            .collect(),
        DashboardLayout::new(vec![
            DashboardLayoutItem::Row(DashboardRow::new(
                RowId::new(0),
                "Before",
                true,
                false,
                vec![],
            )),
            filled_grid(vec![0]),
            filled_grid(vec![1]),
            DashboardLayoutItem::Row(DashboardRow::new(
                RowId::new(1),
                "After",
                true,
                false,
                vec![],
            )),
        ]),
    );
    let area = dashboard_inner_area(Rect::new(0, 0, 140, 48));
    for scroll in [0, 7, 20, usize::MAX] {
        app.vertical_scroll = scroll;
        let (items, _) = projected_dashboard_rects(area, &app);
        assert_eq!(
            items
                .iter()
                .map(|p| (p.rect.y, p.rect.height))
                .collect::<Vec<_>>(),
            vec![(0, 1), (1, 41), (42, 41), (83, 1)]
        );
    }
}
