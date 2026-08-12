# Visual validation contract

Visual validation supplies positive, reviewable evidence for a known behavior.
It joins state assertions, normalized Ratatui buffers, rendered screenshots,
and—only at the interactive boundary—PTY evidence. Do not begin by taking a
screenshot and inventing a reason it looks acceptable.

## Scenario contract

Write each scenario before driving the app, using this required record:

```text
Scenario: concise behavior name
Precondition: deterministic app state and fixture
Viewport: columns × rows
Actions: ordered inputs
Expected state: exact state assertions
Expected screen: text, focus, layout, and style expectations
Evidence: state test, buffer assertion, screenshot, PTY as applicable
Result: pass, fail, or inconclusive with artifact paths
```

The expected screen must be concrete enough to review: identify required and
forbidden text, focus or selection owner, geometry or clipping boundaries,
hierarchy, and meaningful style/contrast. A new screenshot is judged against
these prewritten expectations, not against an improvised visual impression.

## Generate a risk-based scenario matrix

For each change, consider:

- initial state and every material post-input state;
- reversible actions and repeated inputs;
- boundaries, including first/last selection and empty limits;
- populated, empty, loading, and error states when applicable;
- representative normal, narrow, and short viewports;
- affected focus, selection, scrolling, overlays, and mode transitions; and
- nearby code that shares the changed layout or input path.

Choose representative pairs based on risk instead of producing a full
state × input × viewport cross-product. For example, test a narrow viewport
with an open overlay when that combination shares the changed split layout;
do not test every state at every size without a stated risk.

## Evidence and semantic screenshot inspection

Always assert the exact state transition before rendering. Then use a
normalized buffer assertion for stable text, cells, focus styling, cursor, and
layout evidence. Render the same capture with `scripts/render-buffer` when a
human-readable image clarifies the result.

Inspect every required screenshot semantically for:

- clipping, overlap, alignment, and missing content;
- incorrect focus, selection, cursor, or interaction affordance;
- hierarchy and grouping failures;
- contrast or other style mistakes; and
- unexpected artifacts, stale content, or renderer noise.

Screenshots show evidence; they are not an oracle by themselves. Pair them
with state and cell-level checks wherever possible.

## Golden policy

Use a hybrid policy:

- Prefer normalized JSON/cell snapshots for the stable golden because they
  expose semantic diffs and do not depend on font rasterization.
- A screenshot golden may be approved only after it passes the semantic review
  described in the prewritten scenario contract.
- Standardize font, renderer, viewport, theme, timezone/time, and fixture
  data before accepting any image baseline.
- Keep a single golden for each distinct visual state. Do not retain duplicate
  screenshots that represent the same state at the same viewport.
- Update a golden only with an intentional behavior or visual change, an
  updated scenario expectation, a review of the diff, and fresh passing state
  and buffer evidence. Never update merely to hide a failure.

## Viewport matrix

Define explicit dimensions rather than relying on the operator's terminal.
Start with the product's representative normal viewport. Add one narrow case
for width-sensitive layout and one short case for height-sensitive scrolling,
footer, overlay, or clipping risks. Record each chosen `columns × rows` in the
scenario contract and the capture filename. The risk-based matrix determines
whether normal, narrow, or short is relevant.

## PTY boundary

TestBackend proves an in-memory rendered buffer, not a live terminal session.
Use `scripts/pty-smoke` only for an integration claim such as startup,
alternate-screen entry/exit, input encoding, resize, or clean shutdown. Give
the PTY scenario a fixed size, ordered inputs, positive bounded timeout, and
raw ANSI/result artifact paths. Do not make a PTY transcript substitute for a
deterministic buffer assertion.

The PTY steps must reproduce the scenario contract's exact ordered inputs. A
quit-only run proves lifecycle only; it cannot support a keyboard-flow claim.
If the required live state cannot be seeded deterministically, mark that PTY
interaction evidence inapplicable or inconclusive and explain the boundary.
For terminal-restoration claims, compare the relevant terminal attributes
before startup and after exit; cleanup escape sequences or a successful
`disable_raw_mode` call alone do not prove restoration.

If Chromium is missing for PNG rendering, PTY support is unavailable, a
bounded action times out, or input remains nondeterministic, record the result
as `inconclusive`, retain its diagnostics, and do not approve or update a
golden from it.

## Artifacts, retention, and iteration

Keep artifacts under a scenario-specific, reviewable location (for example,
`artifacts/<scenario>/<viewport>/`): the normalized JSON capture, compact
state/buffer assertion output, and only the rendered SVG/PNG or PTY ANSI and
result files needed to explain the outcome. Name files deterministically from
the scenario and dimensions.

Before responding, write the required completion table and `Remaining
uncertainty` line once to `report.md`, then paste that file verbatim into the
final response instead of reconstructing the table.

Avoid artifact bloat: do not commit duplicate visual states, full recordings,
or transient files that add no diagnostic value. Retain failed or inconclusive
artifacts long enough to diagnose and review the outcome; remove superseded
temporary runs once the replacement evidence is accepted according to project
retention practice.

On a failure: preserve the artifacts, identify which written expectation
failed, fix the state/layout/style/input cause, and rerun the same scenario.
Reassert state, recapture the buffer, inspect the fresh screenshot, and repeat
until the scenario passes or is honestly recorded as inconclusive. Do not
replace the baseline before this fix-and-retest loop succeeds.
