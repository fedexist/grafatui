use super::*;

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
