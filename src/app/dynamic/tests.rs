use super::*;
use crate::{
    app::{PanelOptions, PanelType, SeriesView, default_queries},
    dashboard::{
        DashboardItemId, DashboardLayout, DashboardLayoutItem,
        autogrid::{AutoGridOptions, DashboardAutoGrid, test_items},
        variables::{Variable, VariableOption},
    },
    export::ExportOptions,
    theme::Theme,
};
use std::time::Duration;

fn repeated_app(values: &[&str]) -> AppState {
    let mut panels = default_queries(vec![
        "up{node=\"$node\",region=\"$region\"}[$__interval]".into(),
        "up".into(),
    ]);
    panels[0].title = "Load $node".into();
    panels[0].legends = vec![Some("{{instance}}".into())];
    panels[0].panel_type = PanelType::Table;
    panels[0].options = PanelOptions::None;
    panels[1].title = "Summary".into();
    let mut app = AppState::new(
        crate::prom::PromClient::new("http://127.0.0.1:1".into()),
        Duration::from_secs(300),
        Duration::from_secs(15),
        Duration::from_secs(30),
        "Dynamic".into(),
        panels,
        0,
        Theme::from_str("default"),
        "dashed-line".into(),
        ExportOptions::default(),
    );
    app.theme.border_selected = ratatui::style::Color::Cyan;
    app.vars.insert("region".into(), "west".into());
    app.variable_state.scopes[0].variables = vec![Variable {
        name: "node".into(),
        values: values.iter().map(|v| v.to_string()).collect(),
        texts: values.iter().map(|v| v.to_uppercase()).collect(),
        options: vec![
            VariableOption {
                value: "a".into(),
                text: "A".into(),
            },
            VariableOption {
                value: "b".into(),
                text: "B".into(),
            },
        ],
        all: false,
        all_value: Some(".*".into()),
        repeatable: true,
        query: None,
        last_error: None,
    }];
    app.variable_state.panel_scopes = vec![0, 0];
    app.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::AutoGrid(
        DashboardAutoGrid {
            options: AutoGridOptions {
                fit_content: true,
                min_height: Some(0),
                max_height: Some(10),
                min_column_width: 20,
                ..Default::default()
            },
            items: test_items(vec![0, 1]),
        },
    )]));
    app.configure_dynamic(HashMap::from([(
        0,
        AutoGridBehavior {
            repeat: Some("node".into()),
            ..Default::default()
        },
    )]));
    app.view_end_ts = 1_783_080_000;
    app
}
fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn draw(app: &mut AppState, w: u16, h: u16) -> ratatui::buffer::Buffer {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| crate::ui::draw_ui(f, app)).unwrap();
    terminal.backend().buffer().clone()
}

mod conditions;
mod repeats;
mod variables;
