use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn write_dashboard(name: &str, json: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("grafatui-{name}-{stamp}.json"));
    fs::write(&path, json).unwrap();
    path
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("grafana")
        .join(name)
}

fn example_dashboard(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("dashboards")
        .join(name)
}

#[test]
fn validate_strict_exits_nonzero_when_warnings_exist() {
    let path = write_dashboard(
        "strict",
        r#"{
            "title": "Warnings",
            "panels": [
                { "type": "text", "title": "Notes" }
            ]
        }"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_grafatui"))
        .args(["--validate", "--strict", "--grafana-json"])
        .arg(&path)
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("warning[grafana.import.skipped_panel]"));
    assert!(stderr.contains("validation failed with 1 warning(s)"));
}

#[test]
fn validate_strict_accepts_classic_transformations_without_warnings() {
    let path = write_dashboard(
        "classic-transformations",
        r#"{
            "title": "Classic transformations",
            "panels": [{
                "type": "timeseries",
                "title": "CPU",
                "targets": [{"expr": "up"}],
                "transformations": [{"id": "reduce"}]
            }]
        }"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_grafatui"))
        .args([
            "--validate",
            "--strict",
            "--format",
            "json",
            "--grafana-json",
        ])
        .arg(&path)
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let summary: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["panel_count"], 1);
    assert_eq!(summary["diagnostics"], serde_json::json!([]));
}

#[test]
fn validate_json_outputs_machine_readable_summary() {
    let path = write_dashboard(
        "json",
        r#"{
            "title": "JSON Warnings",
            "panels": [
                { "type": "text", "title": "Notes" },
                {
                    "type": "timeseries",
                    "title": "CPU",
                    "targets": [
                        { "expr": "helper_query", "hide": true },
                        { "expr": "visible_query" }
                    ]
                }
            ]
        }"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_grafatui"))
        .args(["--validate", "--format", "json", "--grafana-json"])
        .arg(&path)
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let summary: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(summary["title"], "JSON Warnings");
    assert_eq!(summary["panel_count"], 1);
    assert_eq!(summary["diagnostics"][0]["code"], "skipped_panel");
    assert_eq!(summary["diagnostics"].as_array().unwrap().len(), 1);
}

#[test]
fn validate_accepts_supported_v2_resource_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_grafatui"))
        .args(["--validate", "--format", "json", "--grafana-json"])
        .arg(fixture("v2_compatibility.json"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["title"], "Compatibility");
    assert_eq!(summary["panel_count"], 1);
    assert_eq!(summary["diagnostics"], serde_json::json!([]));
}

#[test]
fn validate_accepts_live_grafana_v2_compatibility_example() {
    let output = Command::new(env!("CARGO_BIN_EXE_grafatui"))
        .args(["--validate", "--format", "json", "--grafana-json"])
        .arg(example_dashboard("grafana_v2_compatibility.json"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let summary: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["title"], "Grafana V2 Compatibility");
    assert_eq!(summary["panel_count"], 2);
    assert_eq!(summary["diagnostics"], serde_json::json!([]));
}

#[test]
fn validate_accepts_live_grafana_v2_rows_example() {
    let output = Command::new(env!("CARGO_BIN_EXE_grafatui"))
        .args(["--validate", "--format", "json", "--grafana-json"])
        .arg(example_dashboard("grafana_v2_rows.json"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["title"], "Grafana V2 Rows");
    assert_eq!(summary["panel_count"], 3);
}

#[test]
fn validate_accepts_v2_rows_layout() {
    let output = Command::new(env!("CARGO_BIN_EXE_grafatui"))
        .args(["--validate", "--format", "json", "--grafana-json"])
        .arg(fixture("v2_rows_layout.json"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["title"], "Rows layout");
    assert_eq!(summary["panel_count"], 3);
}

#[test]
fn validate_rejects_nested_v2_tabs_layout() {
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(fixture("v2_rows_layout.json")).unwrap()).unwrap();
    value["spec"]["layout"]["spec"]["rows"][0]["spec"]["layout"] =
        serde_json::json!({"kind": "TabsLayout", "spec": {}});
    let path = write_dashboard("v2-nested-tabs", &value.to_string());

    let output = Command::new(env!("CARGO_BIN_EXE_grafatui"))
        .args(["--validate", "--grafana-json"])
        .arg(&path)
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("TabsLayout"));
    assert!(stderr.contains("spec.layout.spec.rows[0].spec.layout.kind"));
}

#[test]
fn validate_strict_rejects_v2_unsupported_datasource_warning() {
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(fixture("v2_compatibility.json")).unwrap())
            .unwrap();
    value["spec"]["elements"]["panel-1"]["spec"]["data"]["spec"]["queries"][1]["spec"]["query"]["group"] =
        "loki".into();
    let path = write_dashboard("v2-strict", &value.to_string());

    let output = Command::new(env!("CARGO_BIN_EXE_grafatui"))
        .args(["--validate", "--strict", "--grafana-json"])
        .arg(&path)
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("warning[grafana.import.unsupported_datasource]"));
    assert!(stderr.contains("validation failed with 1 warning(s)"));
}
