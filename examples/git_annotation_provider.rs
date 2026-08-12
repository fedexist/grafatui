use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::process::Command;

#[derive(Deserialize)]
struct Request {
    version: u8,
    range: Range,
}

#[derive(Deserialize)]
struct Range {
    from: String,
    to: String,
}

#[derive(Serialize)]
struct Event {
    time: String,
    text: String,
    tags: Vec<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| format!("could not read request: {error}"))?;
    let request: Request =
        serde_json::from_str(&input).map_err(|error| format!("invalid request: {error}"))?;
    if request.version != 1 {
        return Err(format!("unsupported request version {}", request.version));
    }

    let from = parse_time("range.from", &request.range.from)?;
    let to = parse_time("range.to", &request.range.to)?;
    let repository = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .arg("log")
        .arg(format!("--since={}", from.to_rfc3339()))
        .arg(format!("--until={}", to.to_rfc3339()))
        .arg("--format=%cI%x09%h%x09%s")
        .output()
        .map_err(|error| format!("could not run git: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(if stderr.trim().is_empty() {
            "git log failed".to_string()
        } else {
            stderr.trim_end().to_string()
        });
    }

    let output = String::from_utf8(output.stdout)
        .map_err(|error| format!("git output was not UTF-8: {error}"))?;
    for line in output.lines() {
        let mut fields = line.splitn(3, '\t');
        let time = fields
            .next()
            .filter(|value| !value.is_empty())
            .ok_or("git output missing commit time")?;
        let hash = fields
            .next()
            .filter(|value| !value.is_empty())
            .ok_or("git output missing commit hash")?;
        let subject = fields.next().ok_or("git output missing commit subject")?;
        let event = Event {
            time: time.to_string(),
            text: format!("Commit {hash}: {subject}"),
            tags: vec!["git".to_string(), "commit".to_string()],
        };
        println!(
            "{}",
            serde_json::to_string(&event)
                .map_err(|error| format!("could not encode event: {error}"))?
        );
    }

    Ok(())
}

fn parse_time(field: &str, value: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|error| format!("invalid {field}: {error}"))
}
