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
