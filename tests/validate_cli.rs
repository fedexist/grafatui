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
