---
name: testing-ratatui-tuis
description: Use when implementing, reviewing, or debugging Ratatui/Crossterm terminal interfaces where layout, keyboard or mouse interaction, terminal lifecycle, screenshots, or visual regressions must be validated.
---

# Testing Ratatui TUIs

## Core principle

Build change-specific, deterministic evidence before accepting TUI behavior. Combine state and input assertions, actual Ratatui buffers, semantic image review, and real-terminal checks where the scenario crosses that boundary.

## Workflow

1. Inspect the request, diff, bindings, and existing tests.
2. Before any test, implementation, capture, or PTY command, save each scenario's expectations under its artifact directory using the required record in the [visual validation contract](references/visual-validation.md). Record the path and ordering, then leave this contract unchanged; save outcomes separately.
3. Establish deterministic fixtures, time, theme, dimensions, and application state; add the smallest test seam needed.
4. Assert state transitions and input behavior. Render the asserted state with `TestBackend`, serialize the actual buffer, and follow the [Ratatui harness](references/ratatui-harness.md) for capture and normalization.
5. For every visual state named in the saved scenario contract, run `scripts/render-buffer` on its capture and inspect the resulting image with the available image viewer against the written expectations.
6. Run `scripts/pty-smoke` for critical lifecycle or interaction flows, reproducing the contract's ordered inputs with fixed dimensions, bounded timeout, and retained diagnostics.
7. Diagnose every failure, add a failing regression test before the fix, then rerun affected scenarios and proportionate broader checks.
8. Run the final evidence audit in the [visual validation contract](references/visual-validation.md). Persist only focused tests and approved baselines, not capture helpers.

## Evidence requirements

Map every contracted action and visual state to an artifact; mismatches are `fail`, unavailable evidence is `inconclusive`. Mark `pass` only after all required assertions, buffers, images, PTY flows, lifecycle checks, and broader commands exit successfully.

## Fix loop

Before editing production code, save `<scenario>.red.txt` with `Command: ...`, `Exit: ...`, then raw combined stdout/stderr. If RED is impossible, save the reason to `<scenario>.red-impossible.txt`. Save later results separately. Fix the unmet expectation, then repeat state assertions, capture, image inspection, and applicable PTY flow. Update a baseline only after all evidence agrees.

## Completion report

Save exactly this table, one row per scenario, followed immediately by `Remaining uncertainty`, to `<artifact-root>/report.md`; end the final response with the same content verbatim:

| Scenario | Evidence | Result | Persisted coverage |
| --- | --- | --- | --- |
| `<name>` | state, buffer, image, PTY, broader checks | pass, fail, or inconclusive | test or approved baseline |

Remaining uncertainty: state unavailable capabilities, unstable evidence, and their retained diagnostics.

## Common mistakes

- Starting validation without written scenario expectations.
- Assemble all applicable evidence layers: deterministic state/input assertions, visible buffer checks, model-inspected screenshots, and critical PTY coverage.
- Omitting a model-inspected buffer-derived screenshot.
- Leaving uncertainty unstated after reporting a result.
