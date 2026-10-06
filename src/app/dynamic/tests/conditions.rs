use super::*;

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
