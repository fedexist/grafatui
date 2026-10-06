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
        "up{node=\"$node\",region=\"$region\"}".into(),
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
    app
}
#[test]
fn repeat_reorder_preserves_duplicate_identity_focus_data_and_body_offset() {
    use crate::app::panel_scroll::{PanelBodyIdentity, PanelBodyScroll};
    let mut app = repeated_app(&["a", "b"]);
    assert_eq!(
        app.panels
            .iter()
            .map(|p| p.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Load A", "Load B", "Summary"]
    );
    app.selected_item = Some(DashboardItemId::Panel(1));
    app.panels[1].series = vec![SeriesView {
        name: "kept".into(),
        value: Some(9.0),
        points: vec![(1000., 9.)],
        visible: true,
    }];
    app.panel_body_scroll.insert(
        1,
        PanelBodyScroll {
            identity: PanelBodyIdentity::Table,
            offset: 7,
        },
    );
    let variable = &mut app.variable_state.scopes[0].variables[0];
    variable.values = vec!["b".into(), "a".into(), "b".into()];
    variable.texts = vec!["B".into(), "A".into(), "B".into()];
    app.reconcile_dynamic();
    assert_eq!(
        app.panels
            .iter()
            .map(|p| p.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Load B", "Load A", "Load B", "Summary"]
    );
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
    assert_eq!(app.panels[0].series[0].value, Some(9.));
    assert!(app.panels[2].series.is_empty());
    assert_eq!(app.panel_body_scroll[&0].offset, 7);
    assert!(!app.panel_body_scroll.contains_key(&2));
    let variable = &mut app.variable_state.scopes[0].variables[0];
    variable.values = vec!["b".into()];
    variable.texts = vec!["B".into()];
    app.reconcile_dynamic();
    assert_eq!(app.panels.len(), 2);
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(0)));
    assert_eq!(app.panel_body_scroll.len(), 1);
}
#[test]
fn repeat_all_and_empty_options_use_concrete_values_and_placeholder() {
    let mut app = repeated_app(&["$__all"]);
    app.variable_state.scopes[0].variables[0].all = true;
    app.reconcile_dynamic();
    assert_eq!(
        app.panels
            .iter()
            .map(|p| p.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Load A", "Load B", "Summary"]
    );
    app.variable_state.scopes[0].variables[0].options.clear();
    app.reconcile_dynamic();
    assert_eq!(app.panels[0].title, "Load All");
    assert_eq!(app.panels.len(), 2);
    app.variable_state.scopes[0].variables.clear();
    app.reconcile_dynamic();
    assert_eq!(app.panels[0].title, "Load ");
}
#[tokio::test]
async fn repeat_queries_use_instance_values_without_leaking_to_dashboard() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let mut app = repeated_app(&["a", "b"]);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    app.prometheus =
        crate::prom::PromClient::new(format!("http://{}", listener.local_addr().unwrap()));
    let server = tokio::spawn(async move {
        let mut queries = Vec::new();
        for _ in 0..3 {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = [0; 4096];
            let n = s.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            let url = reqwest::Url::parse(&format!(
                "http://localhost{}",
                req.split_whitespace().nth(1).unwrap()
            ))
            .unwrap();
            queries.push(
                url.query_pairs()
                    .find(|(k, _)| k == "query")
                    .unwrap()
                    .1
                    .to_string(),
            );
            let body = r#"{"status":"success","data":{"resultType":"matrix","result":[]}}"#;
            s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
        }
        queries
    });
    tokio::time::timeout(Duration::from_secs(3), app.refresh())
        .await
        .unwrap()
        .unwrap();
    let mut queries = tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap();
    queries.sort();
    assert_eq!(
        queries,
        vec![
            "up",
            "up{node=\"a\",region=\"west\"}",
            "up{node=\"b\",region=\"west\"}"
        ]
    );
    assert_eq!(
        app.variable_state.scopes[0].variables[0].values,
        vec!["a", "b"]
    );
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
#[tokio::test]
async fn repeat_buffers_follow_reconciliation_and_resize() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::{Rect, Size};
    let mut app = repeated_app(&["a", "b"]);
    app.view_end_ts = 1_783_080_000;
    for (index, panel) in app.panels.iter_mut().take(2).enumerate() {
        panel.series = (1..=20)
            .map(|n| SeriesView {
                name: format!("{}-row{n:02}", if index == 0 { "a" } else { "b" }),
                value: Some(n as f64),
                points: vec![],
                visible: true,
            })
            .collect();
    }
    let initial = draw(&mut app, 120, 32);
    assert!(buffer_text(&initial).contains("Load A"));
    assert!(buffer_text(&initial).contains("Load B"));
    super::super::input::handle_key(
        KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        Size::new(120, 32),
        &mut app,
    )
    .await
    .unwrap();
    assert_eq!(app.selected_item, Some(DashboardItemId::Panel(1)));
    let focused = draw(&mut app, 120, 32);
    let selected = crate::ui::visible_dashboard_rects(Rect::new(0, 0, 120, 32), &app)
        .into_iter()
        .find(|r| r.id == DashboardItemId::Panel(1))
        .unwrap();
    assert_eq!(
        focused[(selected.rect.x, selected.rect.y)].fg,
        ratatui::style::Color::Cyan
    );
    super::super::input::handle_key(
        KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL),
        Size::new(120, 32),
        &mut app,
    )
    .await
    .unwrap();
    assert_eq!(app.panel_body_scroll[&1].offset, 1);
    let scrolled = draw(&mut app, 120, 32);
    assert!(!buffer_text(&scrolled).contains("b-row01"));
    assert!(buffer_text(&scrolled).contains("b-row02"));
    let variable = &mut app.variable_state.scopes[0].variables[0];
    variable.values = vec!["b".into(), "a".into()];
    variable.texts = vec!["B".into(), "A".into()];
    app.reconcile_dynamic();
    assert_eq!(app.selected_panel_index(), Some(0));
    assert_eq!(app.panel_body_scroll[&0].offset, 1);
    let reordered = draw(&mut app, 120, 32);
    assert!(buffer_text(&reordered).contains("b-row02"));
    let variable = &mut app.variable_state.scopes[0].variables[0];
    variable.values = vec!["b".into()];
    variable.texts = vec!["B".into()];
    app.reconcile_dynamic();
    let removed = draw(&mut app, 120, 32);
    assert!(!buffer_text(&removed).contains("Load A"));
    let narrow = draw(&mut app, 40, 20);
    assert!(buffer_text(&narrow).contains("Load B"));
    assert!(!buffer_text(&narrow).contains("Load A"));
    let svg = crate::export::render_svg(&app, Rect::new(0, 0, 40, 20));
    assert!(svg.contains("Load B"));
    assert!(!svg.contains("Load A"));
}
#[test]
fn repeat_recording_tracks_instance_changes_and_noops() {
    let mut app = repeated_app(&["a", "b"]);
    let dir =
        std::env::temp_dir().join(format!("grafatui-dynamic-recording-{}", std::process::id()));
    app.export.dir = dir.clone();
    let area = ratatui::layout::Rect::new(0, 0, 120, 32);
    crate::export::toggle_recording(&mut app, area).unwrap();
    assert_eq!(app.recording.as_ref().unwrap().frames.len(), 1);
    app.reconcile_dynamic();
    crate::export::capture_recording_frame(&mut app, area).unwrap();
    assert_eq!(app.recording.as_ref().unwrap().frames.len(), 1);
    let v = &mut app.variable_state.scopes[0].variables[0];
    v.values = vec!["b".into()];
    v.texts = vec!["B".into()];
    app.reconcile_dynamic();
    crate::export::capture_recording_frame(&mut app, area).unwrap();
    assert_eq!(app.recording.as_ref().unwrap().frames.len(), 2);
    crate::export::stop_recording(&mut app, crate::export::RecordingCompletionReason::Stopped)
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn conditions_hidden_data_instances_refresh_and_return() {
    use crate::dashboard::conditions::{Condition, Conditions, Operator};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let mut app = repeated_app(&["a", "b"]);
    app.dynamic
        .as_mut()
        .unwrap()
        .behaviors
        .get_mut(&0)
        .unwrap()
        .conditions = Some(Conditions {
        show: true,
        all: false,
        items: vec![
            Condition::Variable {
                name: "node".into(),
                operator: Operator::Equals,
                value: "a".into(),
            },
            Condition::Data(true),
        ],
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    app.prometheus =
        crate::prom::PromClient::new(format!("http://{}", listener.local_addr().unwrap()));
    let server = tokio::spawn(async move {
        let mut queries = Vec::new();
        for n in 0..6 {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = [0; 4096];
            let count = s.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..count]);
            let url = reqwest::Url::parse(&format!(
                "http://localhost{}",
                req.split_whitespace().nth(1).unwrap()
            ))
            .unwrap();
            let query = url
                .query_pairs()
                .find(|(k, _)| k == "query")
                .unwrap()
                .1
                .to_string();
            let data = if query.contains("node=\"a\"") || (n >= 3 && query.contains("node=\"b\"")) {
                serde_json::json!([{"metric":{"instance":if query.contains("node=\"a\""){"a-data"}else{"b-data"}},"values":[[1000,"9"]]}])
            } else {
                serde_json::json!([])
            };
            let body=serde_json::json!({"status":"success","data":{"resultType":"matrix","result":data}}).to_string();
            s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
            queries.push(query);
        }
        queries
    });
    app.selected_item = Some(DashboardItemId::Panel(1));
    tokio::time::timeout(Duration::from_secs(3), app.refresh())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(app.visible_panel_indices(), vec![0, 2]);
    assert_eq!(app.selected_panel_index(), Some(0));
    app.view_end_ts = 1_783_080_000;
    let hidden = draw(&mut app, 120, 32);
    assert!(buffer_text(&hidden).contains("Load A"));
    assert!(!buffer_text(&hidden).contains("Load B"));
    assert!(
        !crate::export::render_svg(&app, ratatui::layout::Rect::new(0, 0, 120, 32))
            .contains("Load B")
    );
    app.time_offset = Duration::from_secs(60); // Use a distinct query window even while capture exports take time.
    tokio::time::timeout(Duration::from_secs(3), app.refresh())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        app.visible_panel_indices(),
        vec![0, 1, 2],
        "instances: {:?}; queries: {:?}",
        app.dynamic.as_ref().unwrap().instances,
        app.panels
            .iter()
            .map(|p| (&p.last_url, &p.last_error))
            .collect::<Vec<_>>()
    );
    assert_eq!(app.selected_panel_index(), Some(0));
    app.view_end_ts = 1_783_080_000;
    let restored = draw(&mut app, 120, 32);
    assert!(buffer_text(&restored).contains("Load B"));
    assert!(buffer_text(&restored).contains("b-data"));
    let queries = tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        queries.iter().filter(|q| q.contains("node=\"b\"")).count(),
        2
    );
}

#[test]
fn conditions_parent_activation_and_hidden_only_tab_preserve_query_policy() {
    use crate::dashboard::conditions::{Condition, Conditions};
    use crate::dashboard::{DashboardRow, DashboardTab, DashboardTabs, RowId, TabGroupId};
    let mut app = repeated_app(&["a", "b"]);
    let mut dynamic = app.dynamic.take().unwrap();
    dynamic.behaviors.get_mut(&0).unwrap().conditions = Some(Conditions {
        show: true,
        all: true,
        items: vec![Condition::Data(true)],
    });
    let mut grid = dynamic.layout.items.remove(0);
    if let DashboardLayoutItem::AutoGrid(g) = &mut grid {
        g.items.pop();
    }
    dynamic.layout = DashboardLayout::new(vec![DashboardLayoutItem::Row(DashboardRow::new(
        RowId::new(0),
        "Outer",
        false,
        false,
        vec![DashboardLayoutItem::Tabs(DashboardTabs::new(
            TabGroupId::new(0),
            vec![
                DashboardTab {
                    title: "Repeats".into(),
                    children: vec![grid],
                },
                DashboardTab {
                    title: "Summary".into(),
                    children: vec![DashboardLayoutItem::Panel(1)],
                },
            ],
        ))],
    ))]);
    dynamic.expanded_layout = dynamic.layout.clone();
    dynamic.instances.clear();
    app.panels = dynamic.templates.clone();
    app.layout = dynamic.layout.clone();
    app.dynamic = Some(dynamic);
    app.reconcile_dynamic();
    assert_eq!(app.query_eligible_indices(), vec![0, 1]);
    app.dynamic.as_mut().unwrap().mark_query_result(0, false);
    app.dynamic.as_mut().unwrap().mark_query_result(1, false);
    app.evaluate_dynamic_visibility();
    assert!(app.visible_panel_indices().is_empty());
    assert_eq!(app.query_eligible_indices(), vec![0, 1]);
    let inactive_data = draw(&mut app, 120, 32);
    assert!(!buffer_text(&inactive_data).contains("Load"));
    app.layout.set_active_tab(TabGroupId::new(0), 1).unwrap();
    let active = app.reconcile_dynamic();
    assert_eq!(active, vec![2]);
    assert_eq!(app.query_eligible_indices(), vec![2]);
    assert_eq!(app.visible_panel_indices(), vec![2]);
    let summary_tab = draw(&mut app, 120, 32);
    assert!(buffer_text(&summary_tab).contains("Summary"));
    assert!(!buffer_text(&summary_tab).contains("Load"));
    app.layout.set_row_collapsed(RowId::new(0), true).unwrap();
    app.reconcile_dynamic();
    assert!(app.query_eligible_indices().is_empty());
    assert_eq!(app.panels.len(), 3);
    let collapsed = draw(&mut app, 120, 32);
    assert!(buffer_text(&collapsed).contains("Outer"));
    assert!(!buffer_text(&collapsed).contains("Repeats"));
    app.layout.set_row_collapsed(RowId::new(0), false).unwrap();
    app.layout.set_active_tab(TabGroupId::new(0), 0).unwrap();
    let active = app.reconcile_dynamic();
    assert_eq!(active, vec![0, 1]);
    assert_eq!(app.query_eligible_indices(), vec![0, 1]);
    assert!(app.visible_panel_indices().is_empty());
}
#[test]
fn conditions_repeat_binding_overrides_section_and_dashboard_values() {
    use crate::dashboard::conditions::{Condition, Conditions, Operator};
    use crate::dashboard::variables::VariableScope;
    let mut app = repeated_app(&["a", "b"]);
    let mut local = app.variable_state.scopes[0].variables[0].clone();
    local.values = vec!["x".into(), "y".into()];
    local.texts = vec!["X".into(), "Y".into()];
    let mut region = local.clone();
    region.name = "region".into();
    region.values = vec!["row".into()];
    region.texts = vec!["Row".into()];
    region.repeatable = false;
    app.variable_state.scopes.push(VariableScope {
        parent: Some(0),
        variables: vec![local, region.clone()],
    });
    region.values = vec!["west".into()];
    region.texts = vec!["West".into()];
    app.variable_state.scopes.push(VariableScope {
        parent: Some(1),
        variables: vec![region],
    });
    app.variable_state.panel_scopes[0] = 2;
    let dynamic = app.dynamic.as_mut().unwrap();
    dynamic.templates[0].title = "$region $node".into();
    dynamic.behaviors.get_mut(&0).unwrap().conditions = Some(Conditions {
        show: true,
        all: true,
        items: vec![
            Condition::Variable {
                name: "node".into(),
                operator: Operator::Equals,
                value: "x".into(),
            },
            Condition::Variable {
                name: "region".into(),
                operator: Operator::Equals,
                value: "west".into(),
            },
        ],
    });
    app.reconcile_dynamic();
    assert_eq!(app.panels[0].title, "West X");
    assert_eq!(app.panels[1].title, "West Y");
    assert_eq!(app.visible_panel_indices(), vec![0, 2]);
    assert_eq!(app.query_eligible_indices(), vec![0, 2]);
    let dynamic = app.dynamic.as_ref().unwrap();
    assert_eq!(
        dynamic.values_for(0, &app.variable_state, &app.vars)["node"],
        "x"
    );
    assert_eq!(
        dynamic.values_for(0, &app.variable_state, &app.vars)["region"],
        "west"
    );
    assert_eq!(
        dynamic.values_for(2, &app.variable_state, &app.vars)["region"],
        "west"
    ); // global fallback remains untouched
    assert_eq!(
        app.variable_state.scopes[0].variables[0].values,
        vec!["a", "b"]
    );
}

#[test]
fn conditions_range_reconciles_empty_grid_fullscreen_and_search() {
    use crate::dashboard::conditions::{Condition, Conditions};
    let mut app = repeated_app(&["a", "b"]);
    let condition = Conditions {
        show: true,
        all: true,
        items: vec![Condition::TimeRange("5m".into())],
    };
    let dynamic = app.dynamic.as_mut().unwrap();
    dynamic.behaviors.get_mut(&0).unwrap().conditions = Some(condition.clone());
    dynamic.behaviors.insert(
        1,
        AutoGridBehavior {
            conditions: Some(condition),
            ..Default::default()
        },
    );
    app.reconcile_dynamic();
    assert_eq!(app.visible_panel_indices(), vec![0, 1, 2]);
    let equality = draw(&mut app, 120, 32);
    assert!(buffer_text(&equality).contains("Load A"));
    assert!(buffer_text(&equality).contains("Load B"));
    app.mode = AppMode::FullscreenInspect;
    app.range = Duration::from_secs(301);
    app.evaluate_dynamic_visibility();
    assert!(app.visible_panel_indices().is_empty());
    assert_eq!(app.selected_item, None);
    assert_eq!(app.mode, AppMode::Inspect);
    let empty = draw(&mut app, 120, 32);
    assert!(!buffer_text(&empty).contains("Load A"));
    assert!(!buffer_text(&empty).contains("Load B"));
    assert!(!buffer_text(&empty).contains("Summary"));
    app.mode = AppMode::Search;
    app.search_query = "Load".into();
    app.range = Duration::from_secs(300);
    app.evaluate_dynamic_visibility();
    assert_eq!(
        app.search_results,
        vec![DashboardItemId::Panel(0), DashboardItemId::Panel(1)]
    );
    app.range = Duration::from_secs(301);
    app.evaluate_dynamic_visibility();
    assert!(app.search_results.is_empty());
    assert_eq!(app.mode, AppMode::Search);
}

#[tokio::test]
async fn initial_dynamic_refresh_selects_first_resolved_instance() {
    use crate::dashboard::conditions::{Condition, Conditions, Operator};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let mut app = repeated_app(&["$__all"]);
    let v = &mut app.variable_state.scopes[0].variables[0];
    v.all = true;
    v.options.clear();
    v.query = Some(crate::grafana::TemplateQueryVar {
        name: "node".into(),
        query: "label_values(instance)".into(),
        regex: None,
        query_path: "spec.variables".into(),
    });
    app.dynamic
        .as_mut()
        .unwrap()
        .behaviors
        .get_mut(&0)
        .unwrap()
        .conditions = Some(Conditions {
        show: true,
        all: true,
        items: vec![Condition::Variable {
            name: "node".into(),
            operator: Operator::Matches,
            value: ".+".into(),
        }],
    });
    app.reconcile_dynamic();
    assert_eq!(app.selected_panel_index(), Some(1));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    app.prometheus =
        crate::prom::PromClient::new(format!("http://{}", listener.local_addr().unwrap()));
    let server = tokio::spawn(async move {
        for n in 0..4 {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut b = [0; 4096];
            let _ = s.read(&mut b).await.unwrap();
            let body = if n == 0 {
                r#"{"status":"success","data":["a","b"]}"#
            } else {
                r#"{"status":"success","data":{"resultType":"matrix","result":[]}}"#
            };
            s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
        }
    });
    tokio::time::timeout(Duration::from_secs(3), app.refresh_initial())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(app.selected_panel_index(), Some(0));
    assert_eq!(app.panels[0].title, "Load a");
    tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap();
}

#[test]
fn titles_use_root_overrides_without_overriding_local_labels() {
    use crate::dashboard::variables::VariableScope;
    let mut app = repeated_app(&["a"]);
    let mut env = app.variable_state.scopes[0].variables[0].clone();
    env.name = "env".into();
    env.values = vec!["staging".into()];
    env.texts = vec!["Staging".into()];
    env.repeatable = false;
    app.variable_state.scopes[0].variables.push(env.clone());
    app.variable_state
        .overrides
        .insert("env".into(), "prod".into());
    app.dynamic.as_mut().unwrap().templates[0].title = "Load $node $env".into();
    app.dynamic.as_mut().unwrap().templates[1].title = "Summary $env".into();
    app.reconcile_dynamic();
    assert_eq!(app.panels[0].title, "Load A prod");
    assert_eq!(app.panels[1].title, "Summary prod");
    assert_eq!(
        app.dynamic
            .as_ref()
            .unwrap()
            .values_for(0, &app.variable_state, &app.vars)["env"],
        "prod"
    );
    env.values = vec!["local".into()];
    env.texts = vec!["Local label".into()];
    app.variable_state.scopes.push(VariableScope {
        parent: Some(0),
        variables: vec![env],
    });
    app.variable_state.panel_scopes[0] = 1;
    app.reconcile_dynamic();
    assert_eq!(app.panels[0].title, "Load A Local label");
    assert_eq!(
        app.dynamic
            .as_ref()
            .unwrap()
            .values_for(0, &app.variable_state, &app.vars)["env"],
        "local"
    );
}

#[tokio::test]
async fn local_variable_refresh_obeys_parent_activation_even_when_items_are_hidden() {
    use crate::dashboard::conditions::{Condition, Conditions, Operator};
    use crate::dashboard::variables::VariableScope;
    use crate::dashboard::{DashboardRow, DashboardTab, DashboardTabs, RowId, TabGroupId};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let mut app = repeated_app(&["a"]);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    app.prometheus =
        crate::prom::PromClient::new(format!("http://{}", listener.local_addr().unwrap()));
    let (tx, mut requests) = tokio::sync::mpsc::unbounded_channel();
    let server = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0; 4096];
            let n = socket.read(&mut buf).await.unwrap();
            let request = String::from_utf8_lossy(&buf[..n]);
            let url = reqwest::Url::parse(&format!(
                "http://localhost{}",
                request.split_whitespace().nth(1).unwrap()
            ))
            .unwrap();
            let query = url
                .query_pairs()
                .find(|(k, _)| k == "match[]")
                .unwrap()
                .1
                .to_string();
            let node = if query.contains("first") {
                "first-node"
            } else if query.contains("second") {
                "second-node"
            } else {
                "parent-node"
            };
            tx.send(query).unwrap();
            let body =
                serde_json::json!({"status":"success","data":[{"instance":node}]}).to_string();
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
        }
    });
    let local = |name: &str, section: &str, parent| {
        let mut variable = app.variable_state.scopes[0].variables[0].clone();
        variable.name = name.into();
        variable.query = Some(crate::grafana::TemplateQueryVar {
            name: name.into(),
            query: if section == "parent" {
                "label_values(up{section=\"parent\"}, instance)".into()
            } else {
                format!("label_values(up{{section=\"{section}\",parent=\"$parent\"}}, instance)")
            },
            regex: None,
            query_path: "fixture".into(),
        });
        VariableScope {
            parent: Some(parent),
            variables: vec![variable],
        }
    };
    let scopes = vec![
        local("parent", "parent", 0),
        local("scope", "first", 1),
        local("scope", "second", 1),
    ];
    app.variable_state.scopes.extend(scopes);
    app.variable_state.panel_scopes = vec![2, 3];
    for panel in &mut app.panels {
        panel.exprs.clear();
    }
    let dynamic = app.dynamic.as_mut().unwrap();
    for panel in &mut dynamic.templates {
        panel.exprs.clear();
    }
    dynamic.behaviors.get_mut(&0).unwrap().conditions = Some(Conditions {
        show: true,
        all: true,
        items: vec![Condition::Variable {
            name: "node".into(),
            operator: Operator::Equals,
            value: "never".into(),
        }],
    });
    dynamic.layout = DashboardLayout::new(vec![DashboardLayoutItem::Row(DashboardRow::new(
        RowId::new(0),
        "Outer",
        false,
        false,
        vec![DashboardLayoutItem::Tabs(DashboardTabs::new(
            TabGroupId::new(0),
            vec![
                DashboardTab {
                    title: "First".into(),
                    children: vec![DashboardLayoutItem::AutoGrid(DashboardAutoGrid {
                        options: AutoGridOptions::default(),
                        items: test_items(vec![0]),
                    })],
                },
                DashboardTab {
                    title: "Second".into(),
                    children: vec![DashboardLayoutItem::Panel(1)],
                },
            ],
        ))],
    ))]);
    app.layout = dynamic.layout.clone();
    app.reconcile_dynamic();
    assert!(app.visible_panel_indices().is_empty());
    tokio::time::timeout(Duration::from_secs(3), app.refresh())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(requests.try_recv().unwrap(), "up{section=\"parent\"}");
    assert_eq!(
        requests.try_recv().unwrap(),
        "up{section=\"first\",parent=\"parent-node\"}"
    );
    assert!(
        requests.try_recv().is_err(),
        "inactive tab variable must not be queried"
    );
    assert_eq!(
        app.variable_state.lookup(2, "scope").unwrap().values,
        vec!["first-node"]
    );
    assert_eq!(
        app.variable_state.lookup(3, "scope").unwrap().values,
        vec!["a"]
    );
    tokio::time::timeout(
        Duration::from_secs(3),
        app.activate_tab(TabGroupId::new(0), 1),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(requests.try_recv().unwrap(), "up{section=\"parent\"}");
    assert_eq!(
        requests.try_recv().unwrap(),
        "up{section=\"second\",parent=\"parent-node\"}"
    );
    assert!(requests.try_recv().is_err());
    assert_eq!(
        app.variable_state.lookup(2, "scope").unwrap().values,
        vec!["first-node"]
    );
    assert_eq!(
        app.variable_state.lookup(3, "scope").unwrap().values,
        vec!["second-node"]
    );
    app.layout.set_row_collapsed(RowId::new(0), true).unwrap();
    app.refresh().await.unwrap();
    assert!(requests.try_recv().is_err());
    server.abort();
}
