# Fork-focused hardening for v0.1.14 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Integrate the approved focused hardening from Ryan Craig's fork into current Grafatui while preserving current compatibility and dynamic-layout behavior.

**Architecture:** Integrate six donor slices in dependency order with git cherry-pick -x, resolving against current main and reviewing each slice before the next. JSON and YAML share the existing normalization path; Prometheus safety stays inside PromClient; adaptive intervals carry explicit-versus-automatic policy through query state; terminal and annotation cleanup remain separate lifecycle slices.

**Tech Stack:** Rust 2024, Rust 1.88 minimum, Tokio, Reqwest with rustls, Serde/serde_json, serde-saphyr, Crossterm, Ratatui, Unix nix signal support, Cargo, Git.

**Spec:** docs/superpowers/specs/2026-10-06-fork-focused-hardening-0.1.14-design.md

## Global Constraints

- Start execution in an isolated worktree created with superpowers:using-git-worktrees; base it on the latest accepted main containing spec commits 7b0dcc4 and 245336b.
- Use donor commit code before writing replacement code. Cherry-pick original commits with -x; never cherry-pick fork merge commits.
- Preserve current RowsLayout, TabsLayout, dynamic AutoGrid, variable scopes, repeats, conditions, content sizing, scrolling, exports, and platforms.
- Keep --grafana-json and grafana_json; .yaml/.yml select YAML and every other extension selects strict JSON.
- Explicit CLI/configuration step remains exact. Only an absent step enables automatic calculation.
- Keep rust-version = "1.88"; add only serde-saphyr and the approved Unix-only nix dependency.
- Do not add background refresh, last-good-data, warnings/infos UI, mTLS, theme work, display features, or multiple dashboards.
- 7ef3725 is a donor only for StatusError, is_retryable, and HTTP retry tests. Do not cherry-pick it whole.
- Current TLS handshake failures remain transport-retryable; TLS-specific classification is deferred.
- After each slice, run focused tests, formatting, Clippy, all-target tests, installer tests, and a fresh review.
- Task 5 terminal validation must use the repository testing-ratatui-tuis skill.
- Do not manually bump the package version; release-plz owns it.

## Review Focus

- Missing/null real-export fields may default, but wrong present types must fail at native paths. Task 1 tests both.
- YAML saved with .json or an unknown extension must fail as JSON. Task 2 tests both.
- An explicit step above the automatic point ceiling must remain exact. Task 4 tests both policies.
- Cancelling an in-flight Prometheus leader must release every waiter. Task 3 tests the race.
- Errors and shutdown must not strand terminal state or annotation descendants. Tasks 5 and 6 test cleanup.

---

### Task 1: Integrate real Grafana 13 export compatibility

**Donor:** e79bbd2

**Files:**
- Modify: src/grafana.rs
- Modify: src/grafana/v2.rs
- Create: tests/fixtures/grafana/v2_grafana13_export.json
- Create: tests/fixtures/grafana/v2_grafana13_external_export.json
- Modify: tests/validate_cli.rs
- Modify: docs/grafana-compatibility.md
- Modify: docs/grafana-dashboard-import.md

**Interfaces:**
- Consumes: current detect_and_adapt(Value) and modular v2 AutoGrid/condition/variable adapters.
- Produces: is_prometheus_group, optional_array_from, optional_object_from, optional_string_from, null_as_default, and tolerant typed panel deserialization.
- Preserves: all importer output types and load_grafana_dashboard behavior.

- [ ] **Step 1: Create the worktree and record the baseline**

Use superpowers:using-git-worktrees, then run:

~~~bash
git status --short --branch
cargo test --all-targets
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
~~~

Expected: clean worktree and all commands pass. Record test counts.

- [ ] **Step 2: Cherry-pick the donor**

~~~bash
git cherry-pick -x e79bbd2ed0c4393ac5d6f1f46530c88e37c7af5b
~~~

Expected: importer/test/doc conflicts are acceptable; no unrelated file may enter.

- [ ] **Step 3: Resolve importer conflicts**

Keep current src/grafana/v2/autogrid.rs, conditions.rs, variables.rs, and dynamic fields. Integrate donor defaults/helpers into current src/grafana/v2.rs rather than replacing its newer layout architecture.

Required tests:

- v2_panels_default_fields_that_real_exports_omit.
- v2_panels_accept_absent_or_null_id_and_links.
- rejects_malformed_v2_panel_id_and_links_at_native_paths.
- v2_prometheus_compatible_datasource_groups_import_as_prometheus.
- v2_grafana13_export_imports_server_serialized_nulls.
- v2_grafana13_external_export_resolves_variables_dynamically.
- Existing AutoGrid/dynamic fixture tests remain unchanged.

- [ ] **Step 4: Complete the cherry-pick**

~~~bash
git add src/grafana.rs src/grafana/v2.rs tests/fixtures/grafana tests/validate_cli.rs docs/grafana-compatibility.md docs/grafana-dashboard-import.md
git cherry-pick --continue
~~~

Expected: Ryan remains author and the -x trailer is present.

- [ ] **Step 5: Run focused and slice verification**

~~~bash
cargo test grafana::tests -- --nocapture
cargo test --test validate_cli -- --nocapture
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
tests/install.sh
git diff HEAD^ --check
git status --short
~~~

Expected: all pass and status is clean. Review for overly permissive null handling and deleted newer parser code before Task 2.

---

### Task 2: Integrate strict v2 YAML behind the existing interface

**Donors:** facde3a, b30221e

**Files:**
- Modify: Cargo.toml
- Modify: Cargo.lock
- Modify: src/grafana.rs
- Modify only if donor conflicts require wording: src/cli.rs, src/config.rs
- Create: tests/fixtures/grafana/v2_grafana13_export.yaml
- Modify: tests/validate_cli.rs
- Modify: README.md, ROADMAP.md
- Modify: docs/configuration.md, docs/grafana-compatibility.md, docs/grafana-dashboard-import.md, docs/quick-start.md

**Interfaces:**
- Produces: DocumentFormat::{Json,Yaml}, DocumentFormat::from_path, import_document, parse_document, parse_yaml, ensure_v2_yaml_resource.
- Consumes: Task 1's v2 adapter and existing detect_and_adapt/import::finish.
- Preserves: Args::grafana_json, Config::grafana_json, precedence, and strict JSON for unknown extensions.

- [ ] **Step 1: Cherry-pick the implementation donor**

~~~bash
git cherry-pick -x facde3a97b7003e47616b3d7c0c3c402a79fbc9e
~~~

Expect the donor's --grafana-dashboard alias and DocumentFormat::Detect temporarily; remove both in the separate compatibility-boundary commit below.

- [ ] **Step 2: Resolve current-architecture conflicts and complete the donor**

Keep the donor's source behavior for this commit, including its alias and detection fallback, while resolving around current importer structure. Then run:

~~~bash
git add Cargo.toml Cargo.lock src/grafana.rs src/cli.rs src/config.rs tests/fixtures/grafana tests/validate_cli.rs README.md ROADMAP.md docs
git cherry-pick --continue
cargo test grafana::tests -- --nocapture
cargo test --test validate_cli -- --nocapture
~~~

Expected: donor tests pass and Ryan remains author.

- [ ] **Step 3: Add failing tests for approved adaptations**

Tests:

- v2_yaml_resource_imports_like_its_json_equivalent compares title, queries, variables, layout, diagnostics.
- document_format_follows_file_extension selects YAML only for .yaml/.yml.
- json_files_report_json_errors_without_yaml_fallback.
- unknown_extensions_remain_strict_json.
- yaml_documents_must_be_mappings.
- yaml_requires_exact_v2_resource rejects Classic and wrong versions.
- yaml_rejects_multiple_documents.

Run them before adding adaptation code. Expected: donor fallback/Classic behavior fails the approved contract tests.

- [ ] **Step 4: Apply and commit the approved format boundary**

Remove the donor alias from src/cli.rs and src/config.rs. DocumentFormat::from_path returns Yaml only for case-insensitive yaml/yml; all else is Json. Keep parse_grafana_dashboard strict JSON. parse_yaml requires one mapping. ensure_v2_yaml_resource requires exact dashboard.grafana.app/v2 before detect_and_adapt.

~~~bash
git add src/grafana.rs src/cli.rs src/config.rs tests/validate_cli.rs
git commit -m "fix: keep YAML import compatibility boundary"
~~~

- [ ] **Step 5: Verify the adapted YAML implementation**

~~~bash
cargo test grafana::tests -- --nocapture
cargo test --test validate_cli -- --nocapture
~~~

Expected: focused tests pass and no new CLI/config field exists.

- [ ] **Step 6: Cherry-pick the quick-start donor**

~~~bash
git cherry-pick -x b30221eb8233aec78082a85da7a8aa0c6cb2f988
~~~

Resolve wording to use only --grafana-json.

- [ ] **Step 7: Verify the dependency and slice**

~~~bash
cargo +1.88.0 check --all-targets
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
tests/install.sh
git status --short
~~~

Expected: all pass. Review Cargo.lock for only serde-saphyr's dependency closure before Task 3. If the exact 1.88.0 toolchain is unavailable locally, install it before acceptance or require the same command to pass in CI; do not silently substitute a newer compiler for the minimum-version gate.

---

### Task 3: Integrate Prometheus cancellation, cache, body, and retry hardening

**Primary donor:** ca6a1d2

**Bounded donor:** 7ef3725, only StatusError/is_retryable/error-body parsing/HTTP tests.

**Files:**
- Modify: src/prom.rs
- Modify: src/main.rs

**Interfaces:**
- Produces: CacheKey, bounded QueryCache, InflightGuard, ResponseTooLarge, excerpt, StatusError, is_retryable.
- Preserves: query_range returning Result<Vec<Series>>, instant return types, UI state, and synchronous refresh.
- Adds: forbid(unsafe_code) if the current crate passes.

- [ ] **Step 1: Cherry-pick the primary donor**

~~~bash
git cherry-pick -x ca6a1d228604923bc7e871a03326e3d432bd1f02
~~~

Resolve src/main.rs by retaining current setup and adding only forbid(unsafe_code). Keep APIs added after the fork point.

If the cherry-pick conflicts, finish it before testing:

~~~bash
git add src/main.rs src/prom.rs
git cherry-pick --continue
~~~

- [ ] **Step 2: Run primary donor regressions**

~~~bash
cargo test prom::tests::a_dropped_query_does_not_strand_identical_queries -- --nocapture
cargo test prom::tests::waiters_fail_promptly_when_the_leading_query_is_cancelled -- --nocapture
cargo test prom::tests::the_query_cache_keeps_only_recent_windows -- --nocapture
cargo test prom::tests::oversized_responses_are_rejected -- --nocapture
cargo test prom::tests::error_excerpts_are_truncated_on_character_boundaries -- --nocapture
~~~

Expected: all pass; query_range still returns Vec<Series>.

- [ ] **Step 3: Add failing HTTP retry tests from 7ef3725**

Adapt its counting server without QueryResult/warnings. Tests:

- rejected_queries_are_not_retried: HTTP 400 count is 1 and errorType/error survive.
- too_many_requests_are_retried: HTTP 429 count is 4.
- unavailable_prometheus_is_retried: HTTP 503 count is 4.
- malformed_success_bodies_are_not_retried: invalid JSON count is 1.
- oversized_responses_are_rejected also asserts count 1.
- non_prometheus_error_bodies_keep_an_excerpt.

Expected before code: permanent HTTP/malformed cases show unwanted retries.

- [ ] **Step 4: Apply only approved donor hunks**

Use git show 7ef3725 -- src/prom.rs. Add StatusError, StatusError::new, is_retryable, non-success status parsing, and the !is_retryable loop guard. Keep transport failures retryable. Omit QueryResult, warnings, infos, stale-data, state, and UI changes.

- [ ] **Step 5: Commit the bounded partial port with source credit**

~~~bash
git add src/prom.rs
git commit --author="Ryan Craig <ryan.s.craig@gmail.com>" -m "fix: avoid retrying permanent Prometheus failures

Adapt HTTP retry classification and tests from 7ef3725 without
the deferred last-good-data or warning UI changes."
~~~

- [ ] **Step 6: Run Prometheus and slice gates**

~~~bash
cargo test prom::tests -- --nocapture
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
tests/install.sh
git status --short
~~~

Expected: all pass. Review eviction metadata, guard drop, body allocation, request counts, and absence of warnings/UI changes before Task 4.

---

### Task 4: Integrate adaptive intervals while preserving exact explicit steps

**Donors:** 6971822, b557abe

**Files:**
- Modify: src/app/data.rs, src/app/state.rs, src/app/variables.rs
- Modify current call sites: src/app/dynamic.rs, src/app/event_loop.rs, src/app/input.rs, src/app/mod.rs
- Modify: src/grafana/model.rs, src/grafana/classic.rs, src/grafana/v2.rs, src/grafana/import.rs, src/grafana.rs
- Modify: src/prom.rs, src/main.rs
- Modify affected test constructors in src/export.rs and src/ui/
- Modify: ROADMAP.md, docs/configuration.md, docs/grafana-compatibility.md, docs/grafana-dashboard-import.md

**Interfaces:**
- Produces: QueryIntervals { step, rate_interval }, QueryIntervals::new, parse_min_interval, round_interval.
- Produces: QueryResolution { min_interval, max_data_points, target_min_intervals } and PanelState::query_intervals.
- Adds: StepPolicy::Explicit(Duration), StepPolicy::Automatic { min_step: Duration }, and resolve_step_policy.
- Preserves: explicit step exactly and all current dynamic identity/scoping/eligibility/export behavior.

- [ ] **Step 1: Attempt the main donor cherry-pick**

~~~bash
git cherry-pick -x 6971822aa07eec1b73b561ba3d04ce107df10cd1
~~~

Inventory conflicts. Abort and report if resolution would replace current src/app/dynamic.rs, src/dashboard/, or modular src/grafana/v2/ architecture.

- [ ] **Step 2: Resolve model/import conflicts and complete the donor**

Carry only resolution metadata through current structures:

- model::Panel min_interval and max_data_points.
- model::Target min_interval.
- QueryPanel/PanelState parallel resolution fields.
- Classic panel/target interval and v2 queryOptions interval/maxDataPoints parsing with native-path diagnostics.

Do not take donor copies of current AutoGrid/repeat/condition/variable modules.

Resolve the remaining call sites to the donor's minimum-step semantics, then complete the cherry-pick:

~~~bash
git add src ROADMAP.md docs
git cherry-pick --continue
cargo test app::data::tests -- --nocapture
cargo test app::state::tests -- --nocapture
cargo test grafana::tests -- --nocapture
cargo test app::dynamic::tests -- --nocapture
~~~

Expected: donor behavior and current architecture coexist; Ryan remains author.

- [ ] **Step 3: Add failing explicit-policy tests**

Tests:

- resolve_step_policy_prefers_cli_then_config.
- resolve_step_policy_is_automatic_when_absent.
- explicit_step_is_not_coarsened_past_point_limit.
- automatic_step_stays_within_point_limit.
- target_min_interval_overrides_panel_minimum_in_automatic_mode.
- explicit_step_overrides_imported_minimums.
- Existing scoped-repeat query test still expands local values and interval built-ins correctly.

Expected: donor min_step semantics fail explicit-policy tests before adaptation.

- [ ] **Step 4: Adapt donor intervals to StepPolicy and commit**

Keep donor QueryIntervals/rounding. Route range queries through:

- Explicit(step): exact request step; compute rate interval around it.
- Automatic: range, maxDataPoints, imported minimum, default five seconds, scrape interval, rounding, point ceiling.
- Variable queries: dashboard-level policy without panel metadata.
- Instant queries: no range-step selection.
- expand_expr: selected QueryIntervals so built-ins match the request.

Update constructors mechanically without changing dynamic behavior.

~~~bash
git add src docs/configuration.md docs/grafana-compatibility.md docs/grafana-dashboard-import.md ROADMAP.md
git commit -m "fix: preserve explicit query step semantics"
~~~

- [ ] **Step 5: Run focused policy and compatibility tests**

~~~bash
cargo test app::data::tests -- --nocapture
cargo test app::state::tests -- --nocapture
cargo test grafana::tests -- --nocapture
cargo test app::dynamic::tests -- --nocapture
~~~

- [ ] **Step 6: Cherry-pick millisecond-key follow-up**

~~~bash
git cherry-pick -x b557abeab26b64647faab7b6f35d9d71354bd57a
~~~

Resolve against Task 3's typed CacheKey. Cache and in-flight identities retain full Duration.

- [ ] **Step 7: Run request and slice verification**

~~~bash
cargo test prom::tests::query_range_url_keeps_sub_second_steps -- --nocapture
cargo test app::state::tests::refresh_requests_steps_scaled_to_the_range -- --nocapture
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
tests/install.sh
git status --short
~~~

Expected: all pass. Review explicit policy, metadata alignment, dynamic scopes/eligibility, and absence of background refresh before Task 5.

---

### Task 5: Integrate terminal restoration and recoverable export failures

**Donor:** 33dd30c

**Files:**
- Modify: src/main.rs
- Modify: src/app/event_loop.rs, src/app/input.rs, src/app/mod.rs
- Modify: src/export.rs
- Modify: src/ui/draw.rs

**Interfaces:**
- Produces: TerminalGuard::enter, best-effort restore_terminal, panic-hook restoration.
- Produces: report_export_error in the event loop.
- Preserves: input modes, recording finalization, export geometry, dynamic layout, panel scrolling, annotation exports.

- [ ] **Step 1: Invoke testing-ratatui-tuis and record PTY scenarios**

Record expected state for normal quit, Ctrl-C, application error, export error, and panic unwinding. Verify raw mode, alternate screen, mouse capture, cursor, and process exit.

- [ ] **Step 2: Cherry-pick the donor**

~~~bash
git cherry-pick -x 33dd30ca5999d3c9da923d3d5be1532b8c0eccf6
~~~

Keep current AutoGrid/export layout and panel-scroll code; integrate lifecycle behavior around it.

- [ ] **Step 3: Resolve lifecycle behavior and tests**

TerminalGuard::Drop does not panic. Export/recording failures set export_status and keep the loop alive. Normal quit and Ctrl-C finalize once.

Retain/adapt:

- export_failures_are_reported_instead_of_ending_the_session.
- finalizing_saves_an_active_recording_once.
- ctrl_c_quits_from_every_mode.
- atomic_writes_replace_files_and_leave_nothing_behind_on_failure.
- system_fonts_are_loaded_once.
- export_status_stays_visible_in_a_narrow_footer.

- [ ] **Step 4: Complete and run focused tests**

~~~bash
git add src/main.rs src/app src/export.rs src/ui/draw.rs
git cherry-pick --continue
cargo test app::event_loop::tests -- --nocapture
cargo test export::tests -- --nocapture
cargo test ui::draw::tests::export_status_stays_visible_in_a_narrow_footer -- --nocapture
~~~

- [ ] **Step 5: Run bounded PTY lifecycle checks**

Use the testing skill scenarios. Record commands, exits, terminal evidence, and artifacts. Unit tests alone are insufficient.

- [ ] **Step 6: Run slice gates**

~~~bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
tests/install.sh
git status --short
~~~

Expected: all pass. Review panic-hook chaining, cleanup idempotence, finalization, and geometry before Task 6.

---

### Task 6: Integrate annotation containment, file bounds, and Unix signals

**Donors:** 9205bef, 9337c80

**Files:**
- Modify: Cargo.toml, Cargo.lock
- Modify: src/annotations/command.rs, src/annotations/jsonl.rs
- Modify only if required: src/annotations/model.rs
- Modify: src/main.rs, src/app/event_loop.rs
- Modify: docs/annotations.md

**Interfaces:**
- Produces on Unix: ProcessGroup::led_by, ProcessGroup::kill, drop cleanup using nix.
- Produces: MAX_FILE_BYTES = 16 MiB and JsonlFileProvider::read_limited.
- Produces: ShutdownSignals::register and ShutdownSignals::recv.
- Preserves: current command protocol, stdout/stderr caps, timeouts, snapshot retention, platform fallback, terminal/recording finalization.

- [ ] **Step 1: Cherry-pick process/file hardening**

~~~bash
git cherry-pick -x 9205bef0790e7cf665435b5a44dc6d6267322569
~~~

Retain current iteration-3 protocol and limits; add only missing process-group and file-bound behavior.

- [ ] **Step 2: Resolve and verify annotation tests**

Required:

- timed_out_providers_take_their_background_processes_with_them (Unix).
- successful_providers_leave_no_background_processes (Unix).
- cancelled_refreshes_kill_the_provider_group (Unix).
- Existing direct-child cleanup remains cross-platform.
- oversized_files_are_rejected_without_rereading.
- State-level oversized replacement retains last good snapshot.
- snapshots_keep_only_the_newest_events remains unchanged.

~~~bash
git add Cargo.toml Cargo.lock src/annotations docs/annotations.md
git cherry-pick --continue
cargo test annotations::command::tests -- --nocapture
cargo test annotations::jsonl::tests -- --nocapture
cargo test annotations::tests -- --nocapture
~~~

- [ ] **Step 3: Cherry-pick signal support**

~~~bash
git cherry-pick -x 9337c808826f9820a09792292ad5c17fce749ef8
~~~

Route every signal through Task 5 finalization/TerminalGuard. Do not duplicate listeners or expose Unix APIs on other platforms.

- [ ] **Step 4: Add signal-path coverage**

Assert:

- Ctrl-C exits every mode.
- Unix SIGTERM/SIGHUP cause normal shutdown.
- Active recording finalizes once.
- Terminal restoration occurs after each supported signal.
- Non-Unix builds exclude Unix signal/process-group code.

- [ ] **Step 5: Run cross-platform and slice gates**

~~~bash
cargo check --all-targets
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
tests/install.sh
git status --short
~~~

Require Linux, macOS, and Windows CI before acceptance. Review Unix dependency scoping, termination races, snapshot retention, and cleanup before Task 7.

---

### Task 7: Reconcile documentation and validate the v0.1.14 candidate

**Files:**
- Modify: ROADMAP.md
- Modify as truth requires: README.md and docs/*.md
- Do not modify Cargo.toml/Cargo.lock version fields manually.

**Interfaces:**
- Consumes: six accepted slices.
- Produces: accurate compatibility docs and a release-plz-ready branch.

- [ ] **Step 1: Correct documentation truth**

Set the roadmap current version to 0.1.13 until release-plz creates 0.1.14. Correct AutoGrid statuses. Document real-export defaults, YAML extensions, explicit/automatic steps, Prometheus limits/retries, terminal cleanup, signals, and annotation bounds. Do not claim deferred work.

- [ ] **Step 2: Run JSON/YAML smoke tests**

~~~bash
cargo run -- --validate --strict --grafana-json tests/fixtures/grafana/v2_grafana13_export.json
cargo run -- --validate --strict --grafana-json tests/fixtures/grafana/v2_grafana13_export.yaml
smoke_dir=$(mktemp -d /tmp/grafatui-yaml-smoke.XXXXXX)
cp tests/fixtures/grafana/v2_grafana13_export.yaml "$smoke_dir/dashboard.unknown"
! cargo run -- --validate --strict --grafana-json "$smoke_dir/dashboard.unknown"
~~~

Expected: the first two commands exit zero with equivalent dashboards. The unknown-extension copy fails with JSON context.

- [ ] **Step 3: Run focused hardening suites**

~~~bash
cargo test grafana::tests -- --nocapture
cargo test --test validate_cli -- --nocapture
cargo test prom::tests -- --nocapture
cargo test app::data::tests -- --nocapture
cargo test app::state::tests -- --nocapture
cargo test app::event_loop::tests -- --nocapture
cargo test annotations::tests -- --nocapture
cargo test annotations::command::tests -- --nocapture
cargo test annotations::jsonl::tests -- --nocapture
~~~

- [ ] **Step 4: Run the complete release gate**

~~~bash
cargo +1.88.0 check --all-targets
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
tests/install.sh
cargo package --locked
git diff origin/main...HEAD --check
git status --short --branch
~~~

Expected: all pass and the package contains no temporary artifacts. If the exact 1.88.0 toolchain is unavailable locally, install it before acceptance or require the same command to pass in CI; do not silently substitute a newer compiler for the minimum-version gate.

- [ ] **Step 5: Repeat bounded terminal/signal checks**

Use testing-ratatui-tuis for normal quit, export failure, Ctrl-C, SIGTERM, and SIGHUP. Confirm no child survives timeout/cancellation and no terminal mode remains changed.

- [ ] **Step 6: Commit final documentation if needed**

~~~bash
git add ROADMAP.md README.md docs
git commit -m "docs: prepare focused hardening release"
~~~

Skip if slice commits already contain all correct documentation.

- [ ] **Step 7: Final review and release notes**

Review every donor boundary and origin/main...HEAD as a whole. Release notes credit Ryan Craig and link accepted source commits, list exact hardening, and name deferred work. Confirm release-plz owns the 0.1.14 bump. Stop for user approval before merging any release PR.
