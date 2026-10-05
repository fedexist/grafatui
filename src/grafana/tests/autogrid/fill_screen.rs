use super::*;

#[test]
fn fill_screen_import_accepts_true_without_warnings() {
    let mut json = auto_grid_resource();
    json["spec"]["layout"]["spec"]["fillScreen"] = serde_json::json!(true);
    let imported = parse_grafana_dashboard(&json.to_string()).unwrap();
    assert!(imported.diagnostics.is_empty());
    assert_eq!(imported.layout.visible_panel_indices(), vec![0, 1, 2, 3]);
}

#[test]
fn fill_screen_import_rejects_content_fitting_at_its_own_path() {
    let mut json = auto_grid_resource();
    json["spec"]["layout"]["spec"]["fillScreen"] = serde_json::json!(true);
    json["spec"]["layout"]["spec"]["fitContent"] = serde_json::json!(true);
    let error = parse_grafana_dashboard(&json.to_string())
        .unwrap_err()
        .to_string();
    assert!(error.contains("spec.layout.spec.fitContent"), "{error}");
}

#[test]
fn fill_screen_import_validates_booleans_in_inactive_containers() {
    for bad in [
        serde_json::json!(null),
        serde_json::json!("true"),
        serde_json::json!(1),
    ] {
        for collapsed in [false, true] {
            let mut json = auto_grid_resource();
            let grid = json["spec"]["layout"].clone();
            let mut invalid = grid.clone();
            invalid["spec"]["fillScreen"] = bad.clone();
            let (layout, path) = if collapsed {
                (
                    serde_json::json!({"kind":"RowsLayout","spec":{"rows":[
                        {"kind":"RowsLayoutRow","spec":{"title":"R","collapse":true,"layout":invalid}}
                    ]}}),
                    "spec.layout.spec.rows[0].spec.layout.spec.fillScreen",
                )
            } else {
                (
                    serde_json::json!({"kind":"TabsLayout","spec":{"tabs":[
                        {"kind":"TabsLayoutTab","spec":{"title":"First","layout":grid}},
                        {"kind":"TabsLayoutTab","spec":{"title":"Second","layout":invalid}}
                    ]}}),
                    "spec.layout.spec.tabs[1].spec.layout.spec.fillScreen",
                )
            };
            json["spec"]["layout"] = layout;
            let error = parse_grafana_dashboard(&json.to_string())
                .unwrap_err()
                .to_string();
            assert!(error.contains(path), "{error}");
        }
    }
}

#[test]
fn fill_screen_import_retains_defaults_custom_sizing_and_skipped_indices() {
    for value in [None, Some(false), Some(true)] {
        let mut json = auto_grid_resource();
        if let Some(value) = value {
            json["spec"]["layout"]["spec"]["fillScreen"] = serde_json::json!(value);
        }
        let spec = &mut json["spec"]["layout"]["spec"];
        spec["rowHeightMode"] = serde_json::json!("custom");
        spec["rowHeight"] = serde_json::json!(18.01);
        json["spec"]["elements"]["B"]["spec"]["vizConfig"]["group"] = serde_json::json!("text");
        let imported = parse_grafana_dashboard(&json.to_string()).unwrap();
        assert_eq!(
            auto_grid_options(&imported).fill_screen,
            value.unwrap_or(false)
        );
        assert_eq!(auto_grid_options(&imported).row_height, 2);
        assert_eq!(
            imported
                .queries
                .iter()
                .map(|p| p.title.as_str())
                .collect::<Vec<_>>(),
            vec!["A", "C", "D"]
        );
        assert_eq!(imported.layout.visible_panel_indices(), vec![0, 1, 2]);
        assert_eq!(imported.skipped_panels, 1);
    }
}
