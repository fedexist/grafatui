use super::*;

#[test]
fn content_fit_import_validates_inactive_settings() {
    let mut json = auto_grid_resource();
    let mut grid = json["spec"]["layout"].clone();
    grid["spec"]["fitContent"] = serde_json::json!(true);
    grid["spec"]["minHeightMode"] = serde_json::json!("none");
    json["spec"]["layout"] = serde_json::json!({"kind":"RowsLayout","spec":{"rows":[
        {"kind":"RowsLayoutRow","spec":{"title":"R","collapse":true,"layout":grid}}
    ]}});
    assert!(
        parse_grafana_dashboard(&json.to_string())
            .unwrap()
            .diagnostics
            .is_empty()
    );
    for (field, bad) in [
        ("fitContent", serde_json::json!(null)),
        ("fitContent", serde_json::json!("true")),
        ("matchRowHeights", serde_json::json!(1)),
        ("minHeight", serde_json::json!(1e100)),
    ] {
        let mut invalid = json.clone();
        invalid["spec"]["layout"]["spec"]["rows"][0]["spec"]["layout"]["spec"][field] = bad;
        let error = parse_grafana_dashboard(&invalid.to_string())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!(
                "spec.layout.spec.rows[0].spec.layout.spec.{field}"
            )),
            "{error}"
        );
    }
}

#[test]
fn content_fit_import_retains_overrides_after_skips() {
    for parse_skip in [true, false] {
        let mut json = auto_grid_resource();
        json["spec"]["layout"]["spec"]["fitContent"] = serde_json::json!(true);
        for (i, v) in [(1, false), (2, true), (3, false)] {
            json["spec"]["layout"]["spec"]["items"][i]["spec"]["fitContent"] = serde_json::json!(v);
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
        assert_eq!(
            group
                .items
                .iter()
                .map(|i| (i.index, i.fit_content))
                .collect::<Vec<_>>(),
            vec![(0, None), (1, Some(true)), (2, Some(false))]
        );
        assert_eq!(
            imported
                .queries
                .iter()
                .map(|q| q.title.as_str())
                .collect::<Vec<_>>(),
            vec!["A", "C", "D"]
        );
    }
    let mut json = auto_grid_resource();
    json["spec"]["layout"]["spec"]["items"][1]["spec"]["element"]["name"] = serde_json::json!("A");
    json["spec"]["layout"]["spec"]["items"][1]["spec"]["fitContent"] = serde_json::json!(false);
    let imported = parse_grafana_dashboard(&json.to_string()).unwrap();
    let crate::dashboard::DashboardLayoutItem::AutoGrid(group) = &imported.layout.items[0] else {
        panic!()
    };
    assert_eq!(group.items[0].fit_content, None);
    assert_eq!(group.items[1].fit_content, Some(false));
    assert_eq!(imported.queries[0].title, imported.queries[1].title);
}

#[test]
fn content_fit_import_minima_and_defaults() {
    let imported = parse_grafana_dashboard(&auto_grid_resource().to_string()).unwrap();
    let options = auto_grid_options(&imported);
    assert!(!options.fit_content);
    assert!(options.match_row_heights);
    assert_eq!(options.min_height, None);
    for (mode, height) in [
        ("none", 0),
        ("short", 10),
        ("standard", 18),
        ("tall", 29),
        ("custom", 2),
    ] {
        let mut json = auto_grid_resource();
        let spec = &mut json["spec"]["layout"]["spec"];
        spec["fitContent"] = serde_json::json!(true);
        spec["matchRowHeights"] = serde_json::json!(false);
        spec["minHeightMode"] = serde_json::json!(mode);
        spec["minHeight"] = serde_json::json!(18.01);
        let imported = parse_grafana_dashboard(&json.to_string()).unwrap();
        let o = auto_grid_options(&imported);
        assert!(o.fit_content);
        assert!(!o.match_row_heights);
        assert_eq!(o.min_height, Some(height));
        assert!(imported.diagnostics.is_empty());
    }
    for bad in [
        serde_json::json!(null),
        serde_json::json!("true"),
        serde_json::json!(1),
    ] {
        let mut json = auto_grid_resource();
        json["spec"]["layout"]["spec"]["items"][0]["spec"]["fitContent"] = bad;
        assert!(
            parse_grafana_dashboard(&json.to_string())
                .unwrap_err()
                .to_string()
                .contains("spec.layout.spec.items[0].spec.fitContent")
        );
    }
    for bad in [
        None,
        Some(serde_json::json!(-1)),
        Some(serde_json::json!(null)),
        Some(serde_json::json!(1e100)),
    ] {
        let mut json = auto_grid_resource();
        json["spec"]["layout"]["spec"]["minHeightMode"] = serde_json::json!("custom");
        if let Some(bad) = bad {
            json["spec"]["layout"]["spec"]["minHeight"] = bad;
        }
        assert!(
            parse_grafana_dashboard(&json.to_string())
                .unwrap_err()
                .to_string()
                .contains("spec.layout.spec.minHeight")
        );
    }
}
