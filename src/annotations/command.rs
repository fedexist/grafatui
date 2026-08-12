#![allow(dead_code)]

use chrono::SecondsFormat;
use serde::Serialize;
use std::io;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::{
    AnnotationCommandConfig, AnnotationProvider, AnnotationProviderError, AnnotationRefreshContext,
    AnnotationSnapshot, ProviderFuture, ProviderPoll, jsonl::parse_jsonl,
};

#[derive(Debug)]
pub(crate) struct CommandProvider {
    config: AnnotationCommandConfig,
}

pub(crate) const MAX_STDOUT_BYTES: usize = 10 * 1024 * 1024;
pub(crate) const MAX_STDERR_BYTES: usize = 64 * 1024;

enum CapturedStdout {
    Complete(Vec<u8>),
    LimitExceeded,
}

struct CollectedProcess {
    status: std::process::ExitStatus,
    stdout: io::Result<CapturedStdout>,
    stderr: io::Result<Vec<u8>>,
    stdin: io::Result<()>,
}

enum ProtocolOutcome {
    Complete(CollectedProcess),
    StdoutLimitExceeded,
}

struct ProcessTasks {
    stdin: JoinHandle<io::Result<()>>,
    stdout: JoinHandle<()>,
    stderr: JoinHandle<io::Result<Vec<u8>>>,
}

impl CommandProvider {
    pub(crate) fn new(config: AnnotationCommandConfig) -> Self {
        Self { config }
    }

    async fn run(&self, context: &AnnotationRefreshContext) -> ProviderPoll {
        match self.run_inner(context).await {
            Ok(snapshot) => ProviderPoll::Loaded(snapshot),
            Err(error) => ProviderPoll::Failed(error),
        }
    }

    async fn run_inner(
        &self,
        context: &AnnotationRefreshContext,
    ) -> Result<AnnotationSnapshot, AnnotationProviderError> {
        let request = encode_request(context).map_err(|error| self.error(error))?;
        let mut child = Command::new(&self.config.program)
            .args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| self.error(format!("could not start: {error}")))?;

        let stdin = child
            .stdin
            .take()
            .expect("command configured with piped stdin");
        let stdout = child
            .stdout
            .take()
            .expect("command configured with piped stdout");
        let stderr = child
            .stderr
            .take()
            .expect("command configured with piped stderr");

        let (stdout_sender, stdout_receiver) = oneshot::channel();
        let mut tasks = ProcessTasks {
            stdin: tokio::spawn(write_request(stdin, request)),
            stdout: tokio::spawn(async move {
                let _ = stdout_sender.send(read_stdout_limited(stdout).await);
            }),
            stderr: tokio::spawn(read_stderr_capped(stderr)),
        };

        let protocol = collect_process(&mut child, stdout_receiver, &mut tasks);
        let outcome = match tokio::time::timeout(self.config.timeout, protocol).await {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(reason)) => {
                let cleanup = kill_and_reap(&mut child).await;
                abort_tasks(tasks);
                return Err(self.error(append_cleanup(reason, cleanup)));
            }
            Err(_) => {
                let cleanup = kill_and_reap(&mut child).await;
                abort_tasks(tasks);
                let reason = format!(
                    "timed out after {}",
                    humantime::format_duration(self.config.timeout)
                );
                return Err(self.error(append_cleanup(reason, cleanup)));
            }
        };

        match outcome {
            ProtocolOutcome::StdoutLimitExceeded => {
                let cleanup = kill_and_reap(&mut child).await;
                join_or_abort_tasks(tasks).await;
                Err(self.error(append_cleanup(
                    "exceeded 10 MiB stdout limit".to_string(),
                    cleanup,
                )))
            }
            ProtocolOutcome::Complete(collected) if !collected.status.success() => {
                let reason = match collected.status.code() {
                    Some(code) => format!("exited with status {code}"),
                    None => "terminated by signal".to_string(),
                };
                let reason = match collected.stderr {
                    Ok(stderr) => append_stderr(reason, &stderr),
                    Err(error) => format!("{reason}; could not read stderr: {error}"),
                };
                Err(self.error(reason))
            }
            ProtocolOutcome::Complete(collected) => {
                collected
                    .stdin
                    .map_err(|error| self.error(format!("could not write request: {error}")))?;
                let stdout = collected
                    .stdout
                    .map_err(|error| self.error(format!("could not read stdout: {error}")))?;
                collected
                    .stderr
                    .map_err(|error| self.error(format!("could not read stderr: {error}")))?;
                let CapturedStdout::Complete(stdout) = stdout else {
                    unreachable!("stdout limit is handled before successful completion")
                };
                parse_stdout(&self.config.program, &stdout)
            }
        }
    }

    fn error(&self, reason: impl std::fmt::Display) -> AnnotationProviderError {
        AnnotationProviderError::command(&self.config.program, reason)
    }
}

async fn write_request(mut stdin: tokio::process::ChildStdin, request: Vec<u8>) -> io::Result<()> {
    stdin.write_all(&request).await?;
    stdin.shutdown().await
}

async fn read_stdout_limited(mut stdout: ChildStdout) -> io::Result<CapturedStdout> {
    let mut captured = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let read = stdout.read(&mut chunk).await?;
        if read == 0 {
            return Ok(CapturedStdout::Complete(captured));
        }
        if captured.len() + read > MAX_STDOUT_BYTES {
            return Ok(CapturedStdout::LimitExceeded);
        }
        captured.extend_from_slice(&chunk[..read]);
    }
}

async fn read_stderr_capped(mut stderr: ChildStderr) -> io::Result<Vec<u8>> {
    let mut captured = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let read = stderr.read(&mut chunk).await?;
        if read == 0 {
            return Ok(captured);
        }
        let retained = (MAX_STDERR_BYTES - captured.len()).min(read);
        captured.extend_from_slice(&chunk[..retained]);
    }
}

fn stderr_excerpt(bytes: &[u8]) -> Option<String> {
    let decoded = String::from_utf8_lossy(bytes);
    let mut excerpt = String::new();
    let mut characters = 0;
    let mut separating = false;

    for character in decoded.chars() {
        if character.is_whitespace() || character.is_control() {
            separating = !excerpt.is_empty();
            continue;
        }
        if separating {
            if characters == 512 {
                break;
            }
            excerpt.push(' ');
            characters += 1;
            separating = false;
        }
        if characters == 512 {
            break;
        }
        excerpt.push(character);
        characters += 1;
    }

    (!excerpt.is_empty()).then_some(excerpt)
}

fn append_stderr(reason: String, stderr: &[u8]) -> String {
    match stderr_excerpt(stderr) {
        Some(excerpt) => format!("{reason}; stderr: {excerpt}"),
        None => reason,
    }
}

async fn collect_process(
    child: &mut Child,
    mut stdout_receiver: oneshot::Receiver<io::Result<CapturedStdout>>,
    tasks: &mut ProcessTasks,
) -> Result<ProtocolOutcome, String> {
    let wait = child.wait();
    tokio::pin!(wait);

    let (status, stdout) = tokio::select! {
        stdout = &mut stdout_receiver => {
            let stdout = stdout.map_err(|_| "stdout reader stopped unexpectedly".to_string())?;
            if matches!(stdout, Ok(CapturedStdout::LimitExceeded)) {
                return Ok(ProtocolOutcome::StdoutLimitExceeded);
            }
            let status = wait
                .await
                .map_err(|error| format!("could not wait for completion: {error}"))?;
            (status, stdout)
        }
        status = &mut wait => {
            let status = status
                .map_err(|error| format!("could not wait for completion: {error}"))?;
            let stdout = stdout_receiver
                .await
                .map_err(|_| "stdout reader stopped unexpectedly".to_string())?;
            if matches!(stdout, Ok(CapturedStdout::LimitExceeded)) {
                return Ok(ProtocolOutcome::StdoutLimitExceeded);
            }
            (status, stdout)
        }
    };

    let stdin = (&mut tasks.stdin)
        .await
        .map_err(|error| format!("stdin writer task failed: {error}"))?;
    (&mut tasks.stdout)
        .await
        .map_err(|error| format!("stdout reader task failed: {error}"))?;
    let stderr = (&mut tasks.stderr)
        .await
        .map_err(|error| format!("stderr reader task failed: {error}"))?;

    Ok(ProtocolOutcome::Complete(CollectedProcess {
        status,
        stdout,
        stderr,
        stdin,
    }))
}

async fn kill_and_reap(child: &mut Child) -> Result<(), String> {
    let kill_error = child.kill().await.err();
    let wait_error = child.wait().await.err();
    match (kill_error, wait_error) {
        (None, None) => Ok(()),
        (Some(kill), None) if kill.kind() == io::ErrorKind::InvalidInput => Ok(()),
        (Some(kill), None) => Err(format!("could not kill child: {kill}")),
        (None, Some(wait)) => Err(format!("could not reap child: {wait}")),
        (Some(kill), Some(wait)) => Err(format!(
            "could not kill child: {kill}; could not reap child: {wait}"
        )),
    }
}

fn append_cleanup(reason: String, cleanup: Result<(), String>) -> String {
    match cleanup {
        Ok(()) => reason,
        Err(error) => format!("{reason}; cleanup failed: {error}"),
    }
}

fn abort_tasks(tasks: ProcessTasks) {
    tasks.stdin.abort();
    tasks.stdout.abort();
    tasks.stderr.abort();
}

async fn join_or_abort_tasks(mut tasks: ProcessTasks) {
    let joined = tokio::time::timeout(Duration::from_millis(100), async {
        let _ = (&mut tasks.stdin).await;
        let _ = (&mut tasks.stdout).await;
        let _ = (&mut tasks.stderr).await;
    })
    .await;
    if joined.is_err() {
        abort_tasks(tasks);
    }
}

impl AnnotationProvider for CommandProvider {
    fn refresh<'a>(&'a mut self, context: &'a AnnotationRefreshContext) -> ProviderFuture<'a> {
        Box::pin(self.run(context))
    }
}

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
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;
    use std::time::Duration;

    use super::{
        CommandProvider, MAX_STDERR_BYTES, MAX_STDOUT_BYTES, encode_request, parse_stdout,
    };
    use crate::annotations::{
        AnnotationCommandConfig, AnnotationProvider, AnnotationRefreshContext, ProviderPoll,
    };

    fn compiled_fixture() -> PathBuf {
        static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
        FIXTURE
            .get_or_init(|| {
                let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/annotation_provider.rs");
                let output = std::env::temp_dir().join(format!(
                    "grafatui-annotation-provider-fixture-{}{}",
                    std::process::id(),
                    std::env::consts::EXE_SUFFIX,
                ));
                let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
                let status = std::process::Command::new(rustc)
                    .arg(source)
                    .arg("-o")
                    .arg(&output)
                    .status()
                    .expect("compile annotation provider fixture");
                assert!(status.success());
                output
            })
            .clone()
    }

    fn fixture_config<const N: usize>(
        args: [&str; N],
        timeout: Duration,
    ) -> AnnotationCommandConfig {
        AnnotationCommandConfig {
            program: compiled_fixture().to_string_lossy().into_owned(),
            args: args.into_iter().map(str::to_string).collect(),
            timeout,
        }
    }

    fn fixture_provider<const N: usize>(args: [&str; N], timeout: Duration) -> CommandProvider {
        CommandProvider::new(fixture_config(args, timeout))
    }

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

    #[tokio::test]
    async fn command_provider_writes_request_closes_stdin_and_parses_stdout() {
        let mut provider = fixture_provider(["echo-request"], Duration::from_secs(2));
        let context = fixed_context();

        let ProviderPoll::Loaded(snapshot) = provider.refresh(&context).await else {
            panic!("expected a loaded snapshot");
        };
        assert_eq!(
            snapshot.events()[0].text,
            String::from_utf8(encode_request(&context).unwrap()).unwrap()
        );
    }

    #[tokio::test]
    async fn command_provider_preserves_args_cwd_and_environment() {
        let inherited_key = "PATH";
        let inherited_value = std::env::var(inherited_key).expect("test process must have PATH");
        let expected_cwd = std::env::current_dir().unwrap();
        let mut provider = fixture_provider(
            ["show-context", "--environment", "prod", inherited_key],
            Duration::from_secs(2),
        );

        let ProviderPoll::Loaded(snapshot) = provider.refresh(&fixed_context()).await else {
            panic!("expected a loaded snapshot");
        };
        let text = &snapshot.events()[0].text;
        assert!(text.contains("--environment|prod"));
        assert!(text.contains(&expected_cwd.display().to_string()));
        assert!(text.contains(&inherited_value));
    }

    #[tokio::test]
    async fn command_provider_loads_empty_snapshot_from_empty_stdout() {
        let mut provider = fixture_provider(["empty"], Duration::from_secs(2));

        let ProviderPoll::Loaded(snapshot) = provider.refresh(&fixed_context()).await else {
            panic!("expected a loaded snapshot");
        };
        assert_eq!(snapshot.len(), 0);
    }

    #[tokio::test]
    async fn command_provider_times_out_and_reaps_child() {
        let mut provider = fixture_provider(["sleep", "5000"], Duration::from_millis(50));
        let started = std::time::Instant::now();
        let ProviderPoll::Failed(error) = provider.refresh(&fixed_context()).await else {
            panic!("expected timeout failure");
        };
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(error.to_string().contains("timed out after 50ms"));
    }

    #[tokio::test]
    async fn command_provider_rejects_oversized_stdout() {
        let stdout_bytes = (MAX_STDOUT_BYTES + 1).to_string();
        let mut provider =
            fixture_provider(["stdout-bytes", &stdout_bytes], Duration::from_secs(2));
        let ProviderPoll::Failed(error) = provider.refresh(&fixed_context()).await else {
            panic!("expected stdout limit failure");
        };
        assert!(error.to_string().contains("10 MiB stdout limit"));
    }

    #[tokio::test]
    async fn command_provider_bounds_and_normalizes_failure_stderr() {
        let stderr_bytes = (MAX_STDERR_BYTES + 1024).to_string();
        let mut provider =
            fixture_provider(["stderr-bytes", &stderr_bytes, "7"], Duration::from_secs(2));
        let ProviderPoll::Failed(error) = provider.refresh(&fixed_context()).await else {
            panic!("expected exit failure");
        };
        let message = error.to_string();
        assert!(message.contains("exited with status 7"));
        assert!(message.contains("eeeeeeee"));
        assert!(!message.contains('\n'));
        assert!(message.len() < 1024);
    }

    #[tokio::test]
    async fn command_provider_normalizes_multiline_exit_diagnostic_and_discards_stdout() {
        let mut provider = fixture_provider(["exit", "9"], Duration::from_secs(2));

        let ProviderPoll::Failed(error) = provider.refresh(&fixed_context()).await else {
            panic!("expected exit failure");
        };
        let message = error.to_string();
        assert!(message.contains("exited with status 9"));
        assert!(message.contains("first diagnostic line second diagnostic line"));
        assert!(!message.contains("invalid annotation JSONL"));
        assert!(!message.contains('\n'));
    }

    #[tokio::test]
    async fn command_provider_ignores_stderr_after_successful_exit() {
        let mut provider = fixture_provider(["stderr-bytes", "32", "0"], Duration::from_secs(2));

        let ProviderPoll::Loaded(snapshot) = provider.refresh(&fixed_context()).await else {
            panic!("expected successful empty snapshot");
        };
        assert_eq!(snapshot.len(), 0);
    }

    #[tokio::test]
    async fn command_provider_reports_spawn_failure_without_arguments() {
        let secret = "secret-looking-argument-never-report-this";
        let missing = std::env::temp_dir()
            .join(format!("grafatui-missing-provider-{}", std::process::id()))
            .to_string_lossy()
            .into_owned();
        let mut provider = CommandProvider::new(AnnotationCommandConfig {
            program: missing.clone(),
            args: vec![secret.to_string()],
            timeout: Duration::from_secs(2),
        });

        let ProviderPoll::Failed(error) = provider.refresh(&fixed_context()).await else {
            panic!("expected spawn failure");
        };
        let message = error.to_string();
        assert!(message.contains(&format!("annotation command {missing}")));
        assert!(message.contains("could not start"));
        assert!(!message.contains(secret));
    }

    #[tokio::test]
    async fn command_provider_reports_nonzero_exit_without_arguments() {
        let secret = "secret-looking-argument-never-report-this";
        let mut config = fixture_config(["exit", "7"], Duration::from_secs(2));
        config.args.push(secret.to_string());
        let mut provider = CommandProvider::new(config);

        let ProviderPoll::Failed(error) = provider.refresh(&fixed_context()).await else {
            panic!("expected exit failure");
        };
        let message = error.to_string();
        assert!(message.contains("exited with status 7"));
        assert!(!message.contains(secret));
    }

    #[tokio::test]
    async fn command_provider_reports_malformed_jsonl() {
        let mut provider = fixture_provider(["invalid"], Duration::from_secs(2));

        let ProviderPoll::Failed(error) = provider.refresh(&fixed_context()).await else {
            panic!("expected malformed JSONL failure");
        };
        assert!(error.to_string().contains("annotation command"));
        assert!(error.to_string().contains(":1"));
    }

    #[tokio::test]
    async fn command_provider_reports_invalid_utf8() {
        let mut provider = fixture_provider(["invalid", "utf8"], Duration::from_secs(2));

        let ProviderPoll::Failed(error) = provider.refresh(&fixed_context()).await else {
            panic!("expected invalid UTF-8 failure");
        };
        assert!(error.to_string().contains("stdout must be valid UTF-8"));
    }
}
