use super::super::autogrid::project_auto_grid;
use crate::dashboard::autogrid::{AutoGridItem, AutoGridOptions, test_items};

#[test]
fn content_fit_solver_bounds_overrides_and_wide_coordinates() {
    let mut options = AutoGridOptions {
        fit_content: true,
        min_height: Some(0),
        match_row_heights: false,
        ..Default::default()
    };
    let items = test_items(vec![0, 1, 2, 3]);
    let measure = |index, _| [Some(7), Some(24), None, Some(3)][index];
    for (width, height, fill, extent, ys) in [
        (98, 41, false, 42, vec![0, 0, 24, 24]),
        (138, 41, false, 27, vec![0, 0, 0, 24]),
        (138, 53, true, 53, vec![0, 0, 0, 30]),
    ] {
        options.fill_screen = fill;
        let p = project_auto_grid(1, width, 0, height, &options, &items, measure);
        assert_eq!(p.content_height, extent);
        assert_eq!(p.panels.iter().map(|p| p.rect.y).collect::<Vec<_>>(), ys);
        assert_eq!(
            p.panels.iter().map(|p| p.content_fit).collect::<Vec<_>>(),
            vec![true, true, false, true]
        );
    }
    options.fill_screen = false;
    for (minimum, want) in [
        (None, vec![18, 24, 18, 18]),
        (Some(29), vec![29, 29, 18, 29]),
        (Some(70_000), vec![70_000, 70_000, 18, 70_000]),
    ] {
        options.min_height = minimum;
        let p = project_auto_grid(0, 98, 70_000, 0, &options, &items, measure);
        assert_eq!(
            p.panels.iter().map(|p| p.rect.height).collect::<Vec<_>>(),
            want
        );
        if minimum == Some(70_000) {
            assert_eq!(p.panels[2].rect.y, 140_000);
            assert_eq!(p.content_height, 140_000);
        }
    }
    options.min_height = Some(0);
    options.fit_content = false;
    let items = [
        AutoGridItem {
            index: 0,
            fit_content: Some(true),
        },
        AutoGridItem {
            index: 1,
            fit_content: None,
        },
        AutoGridItem {
            index: 2,
            fit_content: Some(true),
        },
    ];
    let p = project_auto_grid(0, 135, 0, 0, &options, &items, measure);
    assert_eq!(
        p.panels
            .iter()
            .map(|p| (p.rect.height, p.content_fit))
            .collect::<Vec<_>>(),
        vec![(7, true), (18, false), (18, false)]
    );
    options.fit_content = true;
    let p = project_auto_grid(
        0,
        135,
        0,
        0,
        &options,
        &[AutoGridItem {
            index: 0,
            fit_content: Some(false),
        }],
        measure,
    );
    assert_eq!(
        (p.panels[0].rect.height, p.panels[0].content_fit),
        (18, false)
    );
    let p = project_auto_grid(0, 1, 0, 0, &options, &test_items(vec![0, 1]), |_, width| {
        assert_eq!(width, 1);
        Some(2)
    });
    assert_eq!(p.content_height, 4);
    assert_eq!(p.panels[1].rect.y, 2);
    assert!(
        project_auto_grid(0, 0, 0, 0, &options, &items, measure)
            .panels
            .is_empty()
    );
    assert_eq!(
        project_auto_grid(0, 100, 0, 0, &options, &[], measure).content_height,
        0
    );
    options.fill_screen = true;
    options.fit_content = false;
    let p = project_auto_grid(
        0,
        135,
        0,
        60,
        &options,
        &test_items(vec![0, 1, 2, 3]),
        measure,
    );
    assert_eq!(p.content_height, 60);
    assert_eq!(p.panels[3].rect.y, 30);
    assert_eq!(p.panels[0].rect.height, 18);
}
