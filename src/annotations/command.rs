use chrono::SecondsFormat;
use serde::Serialize;

use super::{
    AnnotationProviderError, AnnotationRefreshContext, AnnotationSnapshot, jsonl::parse_jsonl,
};

#[derive(Serialize)]
struct CommandRequest {
    version: u8,
    range: CommandRange,
}

#[derive(Serialize)]
struct CommandRange {
    from: String,
    to: String,
}

fn encode_request(context: &AnnotationRefreshContext) -> Result<Vec<u8>, AnnotationProviderError> {
    let mut encoded = serde_json::to_vec(&CommandRequest {
        version: 1,
        range: CommandRange {
            from: context.from.to_rfc3339_opts(SecondsFormat::AutoSi, true),
            to: context.to.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        },
    })
    .map_err(|error| {
        AnnotationProviderError::new(format!("could not encode annotation request: {error}"))
    })?;
    encoded.push(b'\n');
    Ok(encoded)
}

fn parse_stdout(
    program: &str,
    stdout: &[u8],
) -> Result<AnnotationSnapshot, AnnotationProviderError> {
    let input = std::str::from_utf8(stdout).map_err(|error| {
        AnnotationProviderError::new(format!(
            "annotation command {program}: stdout must be valid UTF-8: {error}"
        ))
    })?;
    parse_jsonl(&format!("annotation command {program}"), input)
        .map_err(|error| AnnotationProviderError::new(error.to_string()))
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::{encode_request, parse_stdout};
    use crate::annotations::AnnotationRefreshContext;

    fn fixed_context() -> AnnotationRefreshContext {
        AnnotationRefreshContext {
            from: DateTime::parse_from_rfc3339("2026-08-12T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            to: DateTime::parse_from_rfc3339("2026-08-12T10:05:00Z")
                .unwrap()
                .with_timezone(&Utc),
        }
    }

    #[test]
    fn encodes_exact_version_one_request_with_trailing_newline() {
        let context = fixed_context();

        let encoded = encode_request(&context).unwrap();
        assert_eq!(
            String::from_utf8(encoded.clone()).unwrap(),
            concat!(
                r#"{"version":1,"range":{"from":"2026-08-12T10:00:00Z","to":"2026-08-12T10:05:00Z"}}"#,
                "\n"
            )
        );

        let request: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        let request = request.as_object().unwrap();
        assert_eq!(request.len(), 2);
        assert!(request.contains_key("version"));
        assert!(request.contains_key("range"));

        let range = request["range"].as_object().unwrap();
        assert_eq!(range.len(), 2);
        assert!(range.contains_key("from"));
        assert!(range.contains_key("to"));
    }

    #[test]
    fn parses_valid_and_empty_command_snapshots() {
        let valid = parse_stdout(
            "./provider",
            br#"{"time":"2026-08-12T10:02:13.125Z","text":"deploy","tags":["prod"]}
"#,
        )
        .unwrap();
        assert_eq!(valid.len(), 1);
        assert_eq!(valid.events()[0].time.timestamp_subsec_millis(), 125);
        assert_eq!(parse_stdout("./provider", b"").unwrap().len(), 0);
    }

    #[test]
    fn command_parse_errors_identify_program_and_line() {
        let error = parse_stdout("./provider", b"{invalid}\n").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("annotation command ./provider:1")
        );

        let utf8 = parse_stdout("./provider", &[0xff]).unwrap_err();
        assert!(utf8.to_string().contains("valid UTF-8"));
    }
}
