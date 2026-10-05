use super::*;
use crate::dashboard::autogrid::{AutoGridOptions, DashboardAutoGrid, test_items};
use crate::dashboard::{DashboardItemId, DashboardLayout, DashboardLayoutItem};

fn rows(count: usize) -> Vec<SeriesView> {
    (1..=count)
        .map(|i| SeriesView {
            name: format!("row{i:02}"),
            value: Some(i as f64),
            points: vec![],
            visible: true,
        })
        .collect()
}
fn fitted_app() -> AppState {
    let mut app = test_app(ExportOptions {
        dir: test_export_dir("content-fit"),
        format: ExportFormat::Both,
        record_max_frames: 10,
    });
    app.view_end_ts = 1_783_080_000;
    app.panels[0].title = "A".into();
    app.panels[0].panel_type = PanelType::Table;
    app.panels[0].series = rows(20);
    app.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::AutoGrid(
        DashboardAutoGrid {
            options: AutoGridOptions {
                fit_content: true,
                min_height: Some(0),
                match_row_heights: false,
                ..Default::default()
            },
            items: test_items(vec![0]),
        },
    )]));
    app
}

#[test]
fn content_fit_export_keeps_last_measured_row() {
    let mut app = fitted_app();
    let viewport = Rect::new(0, 0, 100, 40);
    let rect = ui::visible_dashboard_rects(viewport, &app)[0].rect;
    assert_eq!(rect, Rect::new(1, 4, 98, 24));
    let svg = render_svg(&app, viewport);
    for i in 1..=20 {
        assert!(svg.contains(&format!("row{i:02}")), "missing row{i:02}");
    }
    assert!(svg.contains(r#"x="10" y="72" width="980" height="432""#));
    let dir = test_export_dir("content-fit-png");
    fs::create_dir_all(&dir).unwrap();
    write_png(&svg, &dir.join("table.png")).unwrap();
    assert!(
        fs::read(dir.join("table.png"))
            .unwrap()
            .starts_with(b"\x89PNG")
    );
    fs::remove_dir_all(dir).unwrap();
    app.vertical_scroll = 3;
    let svg = render_svg(&app, Rect::new(0, 0, 100, 24));
    assert!(svg.contains("row20"));
    assert!(!svg.contains("row01"));
    assert!(!svg.contains("row09"));
    assert!(svg.contains("row10"));
    app.vertical_scroll = 0;
    app.panels[0].series = rows(1);
    app.panels[0].series[0].name = "<name&>".into();
    app.panels[0].series[0].value = None;
    app.panels[0].display.no_value = Some("<missing&>".into());
    let svg = render_svg(&app, viewport);
    assert!(svg.contains("&lt;name&amp;&gt;"));
    assert!(svg.contains("&lt;missing&amp;&gt;"));
    app.panels[0].last_error = Some("<error&>".into());
    let svg = render_svg(&app, viewport);
    assert!(svg.contains("&lt;error&amp;&gt;"));
}

#[test]
fn content_fit_recording_tracks_data_and_resize() {
    let mut app = fitted_app();
    let template = app.panels[0].clone();
    app.panels = (0..4)
        .map(|i| {
            let mut p = template.clone();
            p.title = ["A", "B", "C", "D"][i].into();
            p.series = rows([3, 20, 1, 0][i]);
            if i == 2 {
                p.panel_type = PanelType::Stat;
            }
            p
        })
        .collect();
    let DashboardLayoutItem::AutoGrid(group) = &mut app.layout.items[0] else {
        panic!()
    };
    group.items = test_items(vec![0, 1, 2, 3]);
    let viewport = Rect::new(0, 0, 100, 48);
    for (x, y, w, h) in [
        (10, 72, 490, 126),
        (500, 72, 490, 432),
        (10, 504, 490, 306),
        (500, 504, 490, 54),
    ] {
        assert!(
            render_svg(&app, viewport)
                .contains(&format!(r#"x="{x}" y="{y}" width="{w}" height="{h}""#))
        );
    }
    let dir = app.export.dir.clone();
    toggle_recording(&mut app, viewport).unwrap();
    assert_eq!(app.recording.as_ref().unwrap().frame_count, 1);
    app.panels[1].series = rows(25);
    capture_recording_frame(&mut app, viewport).unwrap();
    assert_eq!(app.recording.as_ref().unwrap().frame_count, 2);
    app.panels[1].series = rows(1);
    capture_recording_frame(&mut app, viewport).unwrap();
    assert_eq!(app.recording.as_ref().unwrap().frame_count, 3);
    app.selected_item = Some(DashboardItemId::Panel(3));
    capture_recording_frame(&mut app, Rect::new(0, 0, 140, 48)).unwrap();
    assert_eq!(app.recording.as_ref().unwrap().frame_count, 4);
    capture_recording_frame(&mut app, Rect::new(0, 0, 140, 48)).unwrap();
    assert_eq!(app.recording.as_ref().unwrap().frame_count, 4);
    stop_recording(&mut app, RecordingCompletionReason::Quit).unwrap();
    fs::remove_dir_all(dir).unwrap();
}
