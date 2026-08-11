---
name: testing-ratatui-tuis
description: Use when implementing, reviewing, or debugging Ratatui/Crossterm terminal interfaces where layout, keyboard or mouse interaction, terminal lifecycle, screenshots, or visual regressions must be validated.
---

# Testing Ratatui TUIs

## Core principle

Build change-specific, deterministic evidence before accepting TUI behavior. Combine state and input assertions, actual Ratatui buffers, semantic image review, and real-terminal checks where the scenario crosses that boundary.

## Workflow

1. Inspect the request, diff, bindings, and existing tests.
2. Write scenario expectations for each changed behavior: deterministic precondition, fixed viewport, ordered inputs, exact state, expected screen, and required evidence. Use the [visual validation contract](references/visual-validation.md) to select a risk-based scenario matrix.
3. Establish deterministic fixtures, time, theme, dimensions, and application state; add the smallest test seam needed.
4. Assert state transitions and input behavior. Render the asserted state with `TestBackend`, serialize the actual buffer, and follow the [Ratatui harness](references/ratatui-harness.md) for capture and normalization.
5. Run `scripts/render-buffer` on each relevant capture and inspect its resulting image with the available image viewer against the written expectations.
6. Run `scripts/pty-smoke` for critical lifecycle or interaction flows, with fixed dimensions, scripted input, bounded timeout, and retained diagnostics.
7. Diagnose every failure, add a failing regression test before the fix, then rerun affected scenarios and proportionate broader checks. Persist only compact tests and approved baselines that add durable value.

## Evidence requirements

Mark a scenario `pass` only with passing state/input assertions, matching deterministic buffers, semantic inspection of new or changed screenshots, no unexplained approved-baseline differences, successful critical PTY flows, clean exit and terminal restoration, and relevant proportionate broader tests. Classify unavailable or unstable required evidence as `inconclusive`, never `pass`.

## Fix loop

Preserve failure artifacts, identify the unmet written expectation, fix the state, layout, style, or input cause, and repeat state assertions, capture, image inspection, and applicable PTY flow. Update a baseline only after the revised scenario expectation and all evidence agree.

## Completion report

| Scenario | Evidence | Result | Persisted coverage |
| --- | --- | --- | --- |
| `<name>` | state, buffer, image, PTY, broader checks | pass, fail, or inconclusive | test or approved baseline |

Remaining uncertainty: state unavailable capabilities, unstable evidence, and their retained diagnostics.

## Common mistakes

- Starting validation without written scenario expectations.
- Stopping at deterministic-state checks, visible-content assertions, or a PTY lifecycle check instead of assembling applicable evidence layers.
- Omitting a model-inspected buffer-derived screenshot.
- Leaving uncertainty unstated after reporting a result.
