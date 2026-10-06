use super::content_fit::{fitted_app, rows};
use super::*;
use crate::app::panel_scroll::{
    BodyScrollAction, reconcile_panel_body_scroll, scroll_selected_body,
};
use crate::dashboard::DashboardLayoutItem;

fn bounded_app() -> AppState {
    let mut app = fitted_app();
    let DashboardLayoutItem::AutoGrid(g) = &mut app.layout.items[0] else {
        panic!()
    };
    g.options.max_height = Some(10);
    app
}
fn image(app: &AppState, viewport: Rect, _label: &str) -> String {
    render_svg(app, viewport)
}

#[test]
fn bounds_scroll_exports_preserve_defaults_and_capture_windows() {
    let mut app = bounded_app();
    let viewport = Rect::new(0, 0, 100, 40);
    assert_eq!(
        ui::visible_dashboard_rects(viewport, &app)[0].rect,
        Rect::new(1, 4, 98, 10)
    );
    let svg = image(&app, viewport, "bounds-export-start");
    for n in 1..=6 {
        assert!(svg.contains(&format!("row{n:02}")));
    }
    assert!(!svg.contains("row07"));
    scroll_selected_body(viewport, &mut app, BodyScrollAction::End);
    let svg = image(&app, viewport, "bounds-export-end");
    for n in 15..=20 {
        assert!(svg.contains(&format!("row{n:02}")));
    }
    for n in 1..=14 {
        assert!(!svg.contains(&format!("row{n:02}")));
    }
    assert!(svg.contains(r#"x="10" y="72" width="980" height="180""#));
    app.mode = AppMode::Fullscreen;
    reconcile_panel_body_scroll(viewport, &mut app);
    let svg = image(&app, viewport, "bounds-export-fullscreen");
    assert_eq!(app.panel_body_scroll[&0].offset, 0);
    assert!(svg.contains("row20"));
    assert!(svg.contains("row01"));
    let dir = test_export_dir("bounds-scroll-png");
    fs::create_dir_all(&dir).unwrap();
    write_png(&svg, &dir.join("frame.png")).unwrap();
    assert!(
        fs::read(dir.join("frame.png"))
            .unwrap()
            .starts_with(b"\x89PNG")
    );
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn bounds_scroll_exports_error_wrap_and_style() {
    let mut app = bounded_app();
    let viewport = Rect::new(0, 0, 12, 24);
    app.panels[0].last_error = Some("one two three four".into());
    let default_svg = image(&app, viewport, "bounds-export-error-narrow");
    assert!(default_svg.contains("one two"));
    app.theme.text = Color::Rgb(4, 5, 6);
    let svg = render_svg(&app, viewport);
    assert!(svg.contains("A - ERROR"));
    for (y, line) in [(105, "one two"), (123, "three"), (141, "four")] {
        assert!(svg.contains(&format!(r##"x="20.00" y="{y}.00" fill="#040506" font-size="13.0" text-anchor="start">{line}</text>"##)),"{svg}");
    }
    app.panels[0].last_error = Some(
        (0..20)
            .map(|n| format!("<error{n:02}&>"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let viewport = Rect::new(0, 0, 100, 40);
    reconcile_panel_body_scroll(viewport, &mut app);
    scroll_selected_body(viewport, &mut app, BodyScrollAction::End);
    app.theme.text = Theme::default().text;
    let svg = image(&app, viewport, "bounds-export-error-end");
    assert!(svg.contains("&lt;error12&amp;&gt;"));
    assert!(svg.contains("&lt;error19&amp;&gt;"));
    assert!(!svg.contains("error11"));
    assert!(!svg.contains("<error"));
    let DashboardLayoutItem::AutoGrid(g) = &mut app.layout.items[0] else {
        panic!()
    };
    g.options.fit_content = false;
    let svg = image(&app, viewport, "bounds-export-error-ordinary");
    assert!(svg.contains(r##"fill="#cc3333""##));
    assert!(!svg.contains("A - ERROR"));
}
#[test]
fn bounds_scroll_recording_tracks_body_noops_and_content_changes() {
    let mut app = bounded_app();
    let viewport = Rect::new(0, 0, 100, 40);
    let dir = app.export.dir.clone();
    toggle_recording(&mut app, viewport).unwrap();
    assert_eq!(app.recording.as_ref().unwrap().frame_count, 1);
    for (action, count) in [
        (BodyScrollAction::Down, 2),
        (BodyScrollAction::End, 3),
        (BodyScrollAction::End, 3),
    ] {
        scroll_selected_body(viewport, &mut app, action);
        capture_recording_frame(&mut app, viewport).unwrap();
        assert_eq!(app.recording.as_ref().unwrap().frame_count, count);
    }
    app.panels[0].series = rows(1);
    reconcile_panel_body_scroll(viewport, &mut app);
    capture_recording_frame(&mut app, viewport).unwrap();
    assert_eq!(app.recording.as_ref().unwrap().frame_count, 4);
    app.panels[0].last_error = Some("one two three four".into());
    reconcile_panel_body_scroll(viewport, &mut app);
    capture_recording_frame(&mut app, viewport).unwrap();
    assert_eq!(app.recording.as_ref().unwrap().frame_count, 5);
    let viewport = Rect::new(0, 0, 12, 24);
    reconcile_panel_body_scroll(viewport, &mut app);
    capture_recording_frame(&mut app, viewport).unwrap();
    assert_eq!(app.recording.as_ref().unwrap().frame_count, 6);
    stop_recording(&mut app, RecordingCompletionReason::Quit).unwrap();
    fs::remove_dir_all(dir).unwrap();
}
