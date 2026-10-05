use super::*;

fn auto_grid_resource() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/grafana/v2_autogrid_layout.json"
    ))
    .unwrap()
}

fn auto_grid_options(imported: &DashboardImport) -> crate::dashboard::autogrid::AutoGridOptions {
    let crate::dashboard::DashboardLayoutItem::AutoGrid(group) = &imported.layout.items[0] else {
        panic!("AutoGrid group was flattened");
    };
    group.options
}

#[test]
fn auto_grid_empty_import_retains_group() {
    let value = minimal_v2_with_layout(serde_json::json!({
        "kind": "AutoGridLayout", "spec": {"items": []}
    }));
    let imported = parse_grafana_dashboard(&value.to_string()).unwrap();
    assert!(imported.queries.is_empty());
    assert_eq!(auto_grid_options(&imported).max_columns, 3);
    assert!(imported.layout.visible_panel_indices().is_empty());
}

#[test]
fn auto_grid_import_keeps_supported_order() {
    let mut json = auto_grid_resource();
    json["spec"]["layout"]["spec"]["items"]
        .as_array_mut()
        .unwrap()
        .pop();
    json["spec"]["elements"]["B"]["spec"]["vizConfig"]["group"] = serde_json::json!("text");
    let imported = parse_grafana_dashboard(&json.to_string()).unwrap();
    assert_eq!(
        imported
            .queries
            .iter()
            .map(|p| p.title.as_str())
            .collect::<Vec<_>>(),
        vec!["A", "C"]
    );
    assert!(imported.queries.iter().all(|p| p.grid.is_none()));
    assert_eq!(imported.layout.visible_panel_indices(), vec![0, 1]);
    assert_eq!(imported.skipped_panels, 1);
    assert_eq!(imported.diagnostics.len(), 1);
}

#[test]
fn auto_grid_import_normalizes_modes_and_fractional_pixels() {
    for (mode, cells) in [("narrow", 20), ("standard", 45), ("wide", 77)] {
        let mut json = auto_grid_resource();
        json["spec"]["layout"]["spec"]["columnWidthMode"] = serde_json::json!(mode);
        let imported = parse_grafana_dashboard(&json.to_string()).unwrap();
        assert_eq!(auto_grid_options(&imported).min_column_width, cells);
    }
    for (mode, cells) in [("short", 10), ("standard", 18), ("tall", 29)] {
        let mut json = auto_grid_resource();
        json["spec"]["layout"]["spec"]["rowHeightMode"] = serde_json::json!(mode);
        let imported = parse_grafana_dashboard(&json.to_string()).unwrap();
        assert_eq!(auto_grid_options(&imported).row_height, cells);
    }
    let mut json = auto_grid_resource();
    let spec = &mut json["spec"]["layout"]["spec"];
    spec["columnWidthMode"] = serde_json::json!("custom");
    spec["columnWidth"] = serde_json::json!(200.01);
    spec["rowHeightMode"] = serde_json::json!("custom");
    spec["rowHeight"] = serde_json::json!(18.01);
    spec["maxColumnCount"] = serde_json::json!(2.5);
    let imported = parse_grafana_dashboard(&json.to_string()).unwrap();
    let options = auto_grid_options(&imported);
    assert_eq!(
        (
            options.max_columns,
            options.min_column_width,
            options.row_height
        ),
        (2, 21, 2)
    );
    assert_eq!(
        imported.diagnostics[0].code,
        "autogrid_column_limit_rounded"
    );
    assert_eq!(
        imported.diagnostics[0].path,
        "spec.layout.spec.maxColumnCount"
    );
}

#[test]
fn auto_grid_import_reports_native_paths_for_malformed_settings() {
    for (field, value) in [
        ("maxColumnCount", serde_json::json!(0.5)),
        ("maxColumnCount", serde_json::json!("3")),
        ("columnWidthMode", serde_json::json!("giant")),
        ("rowHeightMode", serde_json::json!(true)),
        ("columnWidth", serde_json::json!(0)),
        ("rowHeight", serde_json::json!(-1)),
        ("columnWidth", serde_json::json!(1e100)),
        ("rowHeight", serde_json::json!(null)),
        ("fillScreen", serde_json::json!(1)),
        ("fitContent", serde_json::json!("false")),
        ("matchRowHeights", serde_json::json!([])),
        ("minHeightMode", serde_json::json!("bad")),
        ("maxHeightMode", serde_json::json!("bad")),
        ("minHeight", serde_json::json!(-1)),
        ("maxHeight", serde_json::json!("4")),
    ] {
        let mut json = auto_grid_resource();
        json["spec"]["layout"]["spec"][field] = value;
        let error = parse_grafana_dashboard(&json.to_string())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!("spec.layout.spec.{field}")),
            "{field}: {error}"
        );
    }
    for mode in ["columnWidthMode", "rowHeightMode"] {
        let mut json = auto_grid_resource();
        json["spec"]["layout"]["spec"][mode] = serde_json::json!("custom");
        let missing = if mode == "columnWidthMode" {
            "columnWidth"
        } else {
            "rowHeight"
        };
        assert!(
            parse_grafana_dashboard(&json.to_string())
                .unwrap_err()
                .to_string()
                .contains(missing)
        );
    }
}

#[test]
fn auto_grid_import_rejects_deferred_behavior_and_accepts_false_defaults() {
    for (field, value) in [
        ("maxHeightMode", serde_json::json!("short")),
        ("maxHeight", serde_json::json!(30)),
    ] {
        let mut json = auto_grid_resource();
        json["spec"]["layout"]["spec"][field] = value;
        let error = parse_grafana_dashboard(&json.to_string())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("unsupported") && error.contains(&format!("spec.layout.spec.{field}")),
            "{error}"
        );
    }
    for (field, value) in [
        ("repeat", serde_json::json!({})),
        ("conditionalRendering", serde_json::json!({})),
        ("fitContent", serde_json::json!("false")),
    ] {
        let mut json = auto_grid_resource();
        json["spec"]["layout"]["spec"]["items"][0]["spec"][field] = value;
        let error = parse_grafana_dashboard(&json.to_string())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!("spec.layout.spec.items[0].spec.{field}")),
            "{error}"
        );
    }
    let mut json = auto_grid_resource();
    for field in ["fillScreen", "fitContent", "matchRowHeights"] {
        json["spec"]["layout"]["spec"][field] = serde_json::json!(false);
    }
    json["spec"]["layout"]["spec"]["maxHeightMode"] = serde_json::json!("unlimited");
    json["spec"]["layout"]["spec"]["items"][0]["spec"]["fitContent"] = serde_json::json!(false);
    assert!(
        parse_grafana_dashboard(&json.to_string())
            .unwrap()
            .diagnostics
            .is_empty()
    );
}

#[test]
fn auto_grid_import_validates_items_and_references() {
    for (pointer, value, expected_path) in [
        (
            "/spec/layout/spec/items",
            serde_json::json!({}),
            "spec.layout.spec.items",
        ),
        (
            "/spec/layout/spec/items/0",
            serde_json::json!(5),
            "spec.layout.spec.items[0]",
        ),
        (
            "/spec/layout/spec/items/0/kind",
            serde_json::json!("GridLayoutItem"),
            "spec.layout.spec.items[0].kind",
        ),
        (
            "/spec/layout/spec/items/0/spec",
            serde_json::json!([]),
            "spec.layout.spec.items[0].spec",
        ),
        (
            "/spec/layout/spec/items/0/spec/element/kind",
            serde_json::json!("Panel"),
            "spec.layout.spec.items[0].spec.element.kind",
        ),
        (
            "/spec/layout/spec/items/0/spec/element/name",
            serde_json::json!("missing"),
            "spec.layout.spec.items[0].spec.element.name",
        ),
    ] {
        let mut json = auto_grid_resource();
        *json.pointer_mut(pointer).unwrap() = value;
        let error = parse_grafana_dashboard(&json.to_string())
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected_path), "{error}");
    }
}

#[test]
fn auto_grid_import_validates_inactive_options() {
    let mut json = auto_grid_resource();
    let grid = json["spec"]["layout"].clone();
    let mut invalid = grid.clone();
    invalid["spec"]["columnWidthMode"] = serde_json::json!("bad");
    json["spec"]["layout"] = serde_json::json!({"kind":"TabsLayout","spec":{"tabs":[
        {"kind":"TabsLayoutTab","spec":{"title":"First","layout":grid}},
        {"kind":"TabsLayoutTab","spec":{"title":"Second","layout":invalid}}
    ]}});
    let error = parse_grafana_dashboard(&json.to_string())
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("spec.layout.spec.tabs[1].spec.layout.spec.columnWidthMode"),
        "{error}"
    );
    json["spec"]["layout"]["spec"]["tabs"][1]["spec"]["layout"]["spec"]["columnWidthMode"] =
        serde_json::json!("standard");
    let imported = parse_grafana_dashboard(&json.to_string()).unwrap();
    assert_eq!(imported.queries.len(), 8);
    assert_eq!(imported.layout.visible_panel_indices(), vec![0, 1, 2, 3]);
}

#[test]
fn auto_grid_import_warns_on_unknown_settings() {
    let mut json = auto_grid_resource();
    json["spec"]["layout"]["spec"]["futureSetting"] = serde_json::json!(true);
    json["spec"]["layout"]["spec"]["items"][0]["spec"]["futureItemSetting"] =
        serde_json::json!(true);
    let imported = parse_grafana_dashboard(&json.to_string()).unwrap();
    assert_eq!(imported.diagnostics.len(), 2);
    assert!(
        imported
            .diagnostics
            .iter()
            .all(|d| d.code == "unsupported_autogrid_setting")
    );
    assert_eq!(
        imported.diagnostics[0].path,
        "spec.layout.spec.futureSetting"
    );
    assert_eq!(
        imported.diagnostics[1].path,
        "spec.layout.spec.items[0].spec.futureItemSetting"
    );
}

mod fill_screen;

mod content_fit;
