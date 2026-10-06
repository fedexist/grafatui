use super::*;

// Rejecting a valid cap or losing an item override must fail these import cases.
#[test]
fn bounds_scroll_import_accepts_named_custom_and_unused_maxima() {
    for mode in [
        None,
        Some("unlimited"),
        Some("short"),
        Some("standard"),
        Some("tall"),
        Some("custom"),
    ] {
        let mut json = auto_grid_resource();
        let spec = &mut json["spec"]["layout"]["spec"];
        spec["fitContent"] = serde_json::json!(true);
        spec["minHeightMode"] = serde_json::json!("none");
        if let Some(mode) = mode {
            spec["maxHeightMode"] = serde_json::json!(mode);
        }
        spec["maxHeight"] = serde_json::json!(163);
        let imported = parse_grafana_dashboard(&json.to_string());
        assert!(imported.is_ok(), "mode {mode:?}: {imported:?}");
        assert!(imported.unwrap().diagnostics.is_empty());
    }
}

#[test]
fn bounds_scroll_import_validates_inactive_maxima_at_native_paths() {
    for (field, value) in [
        ("maxHeightMode", serde_json::json!(null)),
        ("maxHeightMode", serde_json::json!("huge")),
        ("maxHeight", serde_json::json!(0)),
        ("maxHeight", serde_json::json!(-1)),
        ("maxHeight", serde_json::json!(null)),
        ("maxHeight", serde_json::json!("180")),
        ("maxHeight", serde_json::json!(1e100)),
    ] {
        let mut json = auto_grid_resource();
        let mut grid = json["spec"]["layout"].clone();
        grid["spec"][field] = value;
        json["spec"]["layout"] = serde_json::json!({"kind":"RowsLayout","spec":{"rows":[
            {"kind":"RowsLayoutRow","spec":{"collapse":true,"layout":grid}}
        ]}});
        let err = parse_grafana_dashboard(&json.to_string())
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(&format!(
                "spec.layout.spec.rows[0].spec.layout.spec.{field}"
            )),
            "{err}"
        );
    }
    let mut json = auto_grid_resource();
    json["spec"]["layout"]["spec"]["maxHeightMode"] = serde_json::json!("custom");
    let err = parse_grafana_dashboard(&json.to_string())
        .unwrap_err()
        .to_string();
    assert!(err.contains("spec.layout.spec.maxHeight"), "{err}");
}

#[test]
fn bounds_scroll_import_retains_normalized_maxima() {
    for (mode, want) in [
        (None, None),
        (Some("unlimited"), None),
        (Some("short"), Some(10)),
        (Some("standard"), Some(18)),
        (Some("tall"), Some(29)),
        (Some("custom"), Some(10)),
    ] {
        let mut json = auto_grid_resource();
        if let Some(mode) = mode {
            json["spec"]["layout"]["spec"]["maxHeightMode"] = serde_json::json!(mode);
        }
        json["spec"]["layout"]["spec"]["maxHeight"] = serde_json::json!(163);
        let imported = parse_grafana_dashboard(&json.to_string()).unwrap();
        assert_eq!(auto_grid_options(&imported).max_height, want);
    }
}

#[test]
fn bounds_scroll_import_caps_keep_override_ownership_through_skips() {
    for parse_skip in [true, false] {
        let mut json = auto_grid_resource();
        json["spec"]["layout"]["spec"]["maxHeightMode"] = serde_json::json!("short");
        json["spec"]["layout"]["spec"]["fitContent"] = serde_json::json!(true);
        for (index, fit) in [(1, false), (2, true), (3, false)] {
            json["spec"]["layout"]["spec"]["items"][index]["spec"]["fitContent"] =
                serde_json::json!(fit);
        }
        if parse_skip {
            json["spec"]["elements"]["B"]["spec"]["vizConfig"]["group"] = serde_json::json!("text");
        } else {
            json["spec"]["elements"]["B"]["spec"]["data"]["spec"]["queries"] =
                serde_json::json!([]);
        }
        let imported = parse_grafana_dashboard(&json.to_string()).unwrap();
        let crate::dashboard::DashboardLayoutItem::AutoGrid(group) = &imported.layout.items[0]
        else {
            panic!()
        };
        assert_eq!(group.options.max_height, Some(10));
        assert_eq!(
            group
                .items
                .iter()
                .map(|i| (i.index, i.fit_content))
                .collect::<Vec<_>>(),
            [(0, None), (1, Some(true)), (2, Some(false))]
        );
        assert_eq!(
            imported
                .queries
                .iter()
                .map(|p| p.title.as_str())
                .collect::<Vec<_>>(),
            ["A", "C", "D"]
        );
    }
}
