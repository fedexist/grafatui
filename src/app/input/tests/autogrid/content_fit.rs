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
