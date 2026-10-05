use super::super::autogrid::project_auto_grid;
use crate::dashboard::autogrid::{AutoGridOptions, test_items};

// Capping only the natural height but not matched/fill heights must fail.
#[test]
fn bounds_scroll_caps_survive_matching_and_fill() {
    let mut o = AutoGridOptions {
        fit_content: true,
        min_height: Some(0),
        max_height: Some(10),
        match_row_heights: false,
        ..Default::default()
    };
    let items = test_items(vec![0, 1, 2, 3]);
    let measure = |i, _| [Some(7), Some(24), None, Some(3)][i];
    for (width, height, fill, matching, want, extent, ys) in [
        (
            98,
            41,
            false,
            false,
            vec![7, 10, 18, 3],
            28,
            vec![0, 0, 10, 10],
        ),
        (
            98,
            41,
            false,
            true,
            vec![10, 10, 18, 10],
            28,
            vec![0, 0, 10, 10],
        ),
        (
            138,
            53,
            true,
            false,
            vec![7, 10, 18, 3],
            53,
            vec![0, 0, 0, 27],
        ),
        (
            138,
            53,
            true,
            true,
            vec![10, 10, 27, 10],
            53,
            vec![0, 0, 0, 27],
        ),
    ] {
        o.fill_screen = fill;
        o.match_row_heights = matching;
        let p = project_auto_grid(1, width, 0, height, &o, &items, measure);
        assert_eq!(
            p.panels.iter().map(|p| p.rect.height).collect::<Vec<_>>(),
            want
        );
        assert_eq!(p.content_height, extent);
        assert_eq!(p.panels.iter().map(|p| p.rect.y).collect::<Vec<_>>(), ys);
    }
    o.fill_screen = false;
    for (min, want) in [
        (None, vec![18, 18, 18, 18]),
        (Some(29), vec![29, 29, 29, 29]),
    ] {
        o.min_height = min;
        let p = project_auto_grid(0, 98, 0, 41, &o, &items, measure);
        assert_eq!(
            p.panels.iter().map(|p| p.rect.height).collect::<Vec<_>>(),
            want
        );
    }
}

#[test]
fn bounds_scroll_overrides_empty_and_wide_extents() {
    use crate::dashboard::autogrid::AutoGridItem;
    let mut o = AutoGridOptions {
        fit_content: false,
        min_height: Some(0),
        max_height: Some(70_000),
        match_row_heights: false,
        ..Default::default()
    };
    let items = [
        AutoGridItem {
            index: 0,
            fit_content: Some(true),
        },
        AutoGridItem {
            index: 1,
            fit_content: Some(false),
        },
        AutoGridItem {
            index: 2,
            fit_content: Some(true),
        },
    ];
    let p = project_auto_grid(0, 98, 70_000, 0, &o, &items, |i, _| {
        [Some(100_000), Some(100_000), None][i]
    });
    assert_eq!(p.content_height, 70_018);
    assert_eq!(
        p.panels
            .iter()
            .map(|p| (p.rect.height, p.content_fit, p.body_scroll))
            .collect::<Vec<_>>(),
        [(70_000, true, true), (18, false, false), (18, false, false)]
    );
    assert_eq!(p.panels[2].rect.y, 140_000);
    o.max_height = Some(10);
    assert!(
        project_auto_grid(0, 0, 0, 1, &o, &items, |_, _| Some(20))
            .panels
            .is_empty()
    );
    assert_eq!(
        project_auto_grid(0, 10, 0, 1, &o, &[], |_, _| Some(20)).content_height,
        0
    );
}
