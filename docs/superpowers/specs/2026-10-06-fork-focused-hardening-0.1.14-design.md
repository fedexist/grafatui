# Fork-focused hardening for v0.1.14

Status: approved for implementation planning.

## Intent

Mine the production-hardening work from Ryan Craig's `ryancraig/grafana-tui`
fork into the current Grafatui architecture without merging the fork or
replacing newer upstream behavior. The work is a focused v0.1.14 candidate:
real Grafana v2 export compatibility, v2 YAML input, Prometheus client
hardening, adaptive query intervals, and terminal/process cleanup.

Compatibility remains the governing principle. Existing Classic and v2 JSON
imports, RowsLayout, TabsLayout, dynamic AutoGrid behavior, CLI/configuration,
query semantics, exports, annotations, and supported platforms must continue to
work. Background refresh, last-known-good query data, mTLS, theme changes,
display/export improvements, and multiple-dashboard support are outside this
release.

## Source-integration policy

Use the fork as the source implementation rather than recreating equivalent
code. Cherry-pick the original feature commits, not their merge commits, with
`git cherry-pick -x` so authorship and provenance remain intact:

| Slice | Source commits |
| --- | --- |
| Real Grafana exports | `e79bbd2` |
| V2 YAML | `facde3a`, `b30221e` |
| Prometheus client hardening | `ca6a1d2`; retry-classification hunks and tests from `7ef3725` |
| Adaptive intervals | `6971822`, `b557abe` |
| Terminal and export cleanup | `33dd30c` |
| Annotation containment and signals | `9205bef`, `9337c80` |

Current `main` is the architectural authority during conflict resolution. A
resolution must not delete, bypass, or regress newer RowsLayout, TabsLayout,
AutoGrid, scoped-variable, repeat, condition, content-sizing, scrolling, or
export behavior.

Attempt each cherry-pick in isolation and run its focused tests before adding
supplementary changes. If a commit cannot fit without reintroducing obsolete
fork architecture, abort it and stop for a user checkpoint. Manual porting is
permitted only after that checkpoint. New code is limited to conflict
resolution, adaptations required by this specification, and regression tests
for those adaptations.

The user approved one bounded partial port in advance: do not cherry-pick
`7ef3725` as a whole because it also introduces deferred last-known-good data,
warning, and UI behavior. Take only its `StatusError`/`is_retryable` logic and
HTTP retry tests in the Prometheus-hardening slice, retaining source credit.

Credit Ryan Craig and link the source commits in the resulting PR descriptions
and release notes. Preserve original authorship for accepted cherry-picks.

## Delivery slices

Integrate six independently reviewable and revertible slices in dependency
order:

1. Real-export compatibility.
2. YAML decoding.
3. Prometheus client hardening.
4. Adaptive query intervals.
5. Terminal and export cleanup.
6. Annotation containment and Unix signal cleanup.

Each slice begins from the latest accepted `main`. It includes its own tests,
documentation, focused verification, and review. Do not mix later fork changes
into an earlier slice merely to reduce conflicts.

Stop and reassess when conflict resolution would remove newer behavior, change
behavior outside this document, alter the Rust 1.88 floor or supported
distribution targets, or require an obsolete fork abstraction.

## Import architecture

Keep the existing `--grafana-json` CLI option and `grafana_json` configuration
key. Do not add, rename, or deprecate an input option in v0.1.14.

`load_grafana_dashboard(path)` selects only the input decoder:

- `.yaml` and `.yml` use YAML decoding.
- `.json` and every unknown extension remain strict JSON.

Both decoders produce a `serde_json::Value` and enter the existing
`detect_and_adapt` then `import::finish` pipeline. YAML input must be a single
mapping with the exact `dashboard.grafana.app/v2` API version. Classic-shaped
YAML, multi-document YAML, scalars, and sequences fail clearly. YAML support
must not create a second importer or a second normalization path.

Real-export tolerance belongs only in the v2 adapter. Fields demonstrated as
absent or `null` by checked-in real Grafana 13 exports may use documented
defaults. A present value of the wrong type remains an error at its native
source path. The importer must continue to distinguish malformed input from an
empty optional container.

Bring over the real server-serialized and external-export fixtures from the
fork. Preserve their source shape rather than reducing them to synthetic
minimal cases. Recognize the Amazon Managed Service for Prometheus and Azure
Managed Prometheus datasource groups as Prometheus-compatible. Improve the
library-panel diagnostic without attempting library-panel resolution.

## Prometheus client architecture

Keep hardening encapsulated in `PromClient` and avoid UI or refresh-scheduling
changes.

Use a typed range-query identity containing expression, start, end, and the
full `Duration` step. The same identity governs both the in-flight map and the
cache, so millisecond steps cannot collide. A leader owns an RAII in-flight
guard. Publishing removes its entry and wakes all waiters; dropping the leader
also removes the entry and closes the waiter channels so cancellation cannot
strand later identical requests.

Retain no more than 64 complete range-query windows. Eviction must be
deterministic and must not leave order metadata for removed entries. Cached
results remain complete results and are cloned for callers; this change does
not introduce partial caching or time-based expiry.

Read at most 64 MiB from a Prometheus response, enforcing the limit through
both `Content-Length` and streamed chunks. Reject an oversized response before
JSON parsing and do not retry it. Error excerpts remain bounded and preserve
UTF-8 boundaries.

Retry only failures that can reasonably succeed without changing the request:
current transport failures, HTTP 429, and HTTP 5xx. Do not retry rejected 4xx
queries, malformed JSON, or oversized responses. TLS-specific transport
classification depends on the deferred TLS subsystem and remains out of scope;
current TLS handshake failures retain transport retry behavior. Retain the
existing retry count and backoff unless the source commit requires a correction
for current dependencies. Keep `#![forbid(unsafe_code)]` if it applies without
exceptions.

## Adaptive query intervals

Represent query-step selection as an explicit policy rather than losing
whether the user supplied a value:

- `Explicit(Duration)` comes from CLI or configuration and remains exact.
- `Automatic` applies only when neither source provides a step.

The existing default five-second resolution becomes the minimum for automatic
selection, not an implicit explicit value. An explicit value may exceed
Prometheus's point ceiling and may be rejected by Prometheus; preserving that
user choice takes precedence over silently changing it.

Automatic range queries derive their actual step from the selected range,
Grafana `maxDataPoints`, panel/query minimum intervals, the five-second default
minimum, and the 11,000-point ceiling. Round automatically derived intervals to
the Grafana-style preferred interval table. Invalid, zero, or unresolved
dashboard interval settings produce the existing diagnostic behavior or fall
back to the automatic defaults; they must not panic.

`$__interval`, `$__interval_ms`, `$__rate_interval`, and
`$__rate_interval_ms` use the actual step sent to Prometheus. Rate interval
uses Grafana's `max(step + scrape interval, 4 * scrape interval)` relationship
with the documented default scrape interval when no source-specific interval
is available. Instant queries remain instant and do not acquire a range step.
Variable queries without panel context use the dashboard-level automatic step.

Carry panel and target resolution metadata through the existing import model
and parallel query fields without weakening current scoped-variable or dynamic
AutoGrid identity. The adaptive fork commit predates current dynamic AutoGrid
work, so any conflict resolution must retain current query eligibility,
instance scopes, visibility evaluation, focus, scrolling, and export behavior.

## Terminal, export, annotation, and signal lifecycle

Introduce a small RAII terminal-session guard after raw mode and alternate
screen entry. It owns best-effort cursor restoration, alternate-screen exit,
and raw-mode disablement on normal return, application error, early return, and
panic unwinding. Explicit cleanup may return an error; `Drop` cleanup must not
panic.

Export and recording actions report failures through existing application
status instead of terminating the event loop. A failed export must not claim
success or corrupt an active recording. Normal quit and signal-driven quit use
the same recording finalization and terminal cleanup path.

Handle SIGINT everywhere. On Unix, also handle SIGTERM and SIGHUP as normal
shutdown requests. Do not add unsupported signal assumptions on Windows.

Retain the current annotation command stdout/stderr limits and timeout
behavior. Add only the missing fork behavior: bound file-source reads before
allocation, retain the last good snapshot after a rejected replacement, and on
Unix place command providers in their own process group so timeout,
cancellation, and shutdown terminate descendants. On other platforms, retain
direct-child `kill_on_drop` behavior.

## Acceptance criteria

### Real exports

- Both real Grafana 13 fixtures import successfully with the expected panels,
  variables, layouts, and diagnostics.
- Only source-demonstrated absent or `null` fields default.
- Wrongly typed present values fail at native paths.
- Managed Amazon and Azure Prometheus query groups import as PromQL sources.
- Classic imports and current v2 dynamic-layout fixtures remain unchanged.

### YAML

- Equivalent v2 JSON and YAML normalize to equivalent dashboards.
- `.yaml` and `.yml` accept a single exact-v2 mapping.
- JSON and unknown extensions remain strict JSON.
- Classic-shaped YAML, multi-document YAML, and non-mapping YAML fail clearly.
- CLI/configuration names and precedence remain unchanged.

### Prometheus hardening

- Cancelling an in-flight leader promptly releases identical waiters.
- Cache length never exceeds 64 complete query windows.
- Millisecond-distinct steps do not share cache or in-flight work.
- Bodies above 64 MiB fail before parsing.
- Retry tests distinguish transport/429/5xx from permanent 4xx, malformed, and
  oversized failures. TLS-specific classification remains deferred.

### Adaptive intervals

- Explicit CLI/configuration steps remain exact.
- Automatic short ranges retain the five-second minimum.
- Automatic long ranges round upward and remain within 11,000 points.
- `maxDataPoints` and panel/target minimums affect only the relevant query.
- Built-in interval variables match the request step.
- Instant, variable, scoped, repeated, and condition-driven query behavior
  retains its current semantics.

### Lifecycle

- Terminal cleanup runs after normal exit, errors, export failures, and panic
  unwinding.
- Export failures remain visible without closing the TUI.
- SIGINT/SIGTERM/SIGHUP follow normal finalization where supported.
- Oversized annotation files do not replace the last good snapshot.
- Timed-out or cancelled annotation commands terminate descendants on Unix and
  at least their direct child elsewhere.

## Verification and review

Before the first slice, record a clean full-suite baseline. After every
cherry-pick, run the source commit's focused tests before supplementary edits.
Every conflict-resolution adaptation receives a regression test that is shown
failing before the adaptation and passing afterward.

Every slice must pass:

- Its focused importer, Prometheus, application, export, annotation, or CLI
  tests.
- `cargo fmt --all -- --check`.
- `cargo clippy --all-targets -- -D warnings`.
- `cargo test --all-targets`.
- `tests/install.sh`.
- Documentation and compatibility-matrix review.
- A fresh slice review before the next slice begins.

Before release, perform a whole-series review, JSON/YAML validation smoke
tests, cancellation/retry/response-limit checks, a bounded PTY terminal-cleanup
check, and package/release workflow checks. Correct the stale roadmap version
and AutoGrid status during the final documentation pass.

Do not manually bump the package version during feature work. Allow
release-plz to accumulate the six accepted slices, and merge its release PR
only after the final gates pass. Release notes must credit the fork and link the
source changes.

## Rollback

Each slice must remain isolated enough to revert without removing unrelated
work; dependency order must be respected when reverting stacked slices. There
are no data migrations or CLI/configuration renames. YAML introduces the only
expected new runtime dependency; if it violates the Rust floor, supported
targets, or packaging, the YAML slice is deferred without blocking the other
hardening work. A failed or deferred later slice does not prevent already
accepted earlier slices from forming a smaller v0.1.14.
