# External Annotations

Grafatui can overlay read-only, external point events from one JSONL file. It
never edits or writes the source file. External JSONL is deliberately separate
from Grafana dashboard annotations: Grafatui does not implement Grafana
annotation queries, APIs, `annotations`, or `annotations.list`.

## Enable Annotations

Pass the source path on the command line:

```bash
grafatui \
  --grafana-json ./dashboard.json \
  --annotations-file ./events.jsonl
```

Or configure the same single source in TOML:

```toml
annotations_file = "./events.jsonl"
```

The CLI option overrides the configured path. This source is opt-in and
read-only; Grafatui does not create, edit, or otherwise write the file.

## JSONL Event Format and Targeting

The file contains one JSON object per line. Blank lines are ignored. Each event
requires `time` to be an RFC3339 string with an explicit timezone or offset
(numeric timestamps are rejected) and non-empty `text`. `tags` is an optional
array of non-empty strings.

```json
{"time":"2026-07-23T14:30:00Z","text":"Maintenance window","tags":["maintenance"]}
{"time":"2026-07-23T14:30:00Z","text":"Deployed v2.4","tags":["deploy","production"],"panel_titles":["HTTP Request Rate by Status Code"]}
```

Omit `panel_titles` to target all eligible graph and timeseries panels, as in
the first event. When `panel_titles` is present, it must contain one or more
non-blank titles and each title is matched exactly and case-sensitively against
eligible graph/timeseries panel titles. `null`, an empty array, and blank
titles are validation errors.

If a title occurs on multiple eligible panels, the event fans out to all of
them and Grafatui shows one warning for that duplicate title. A title that is
missing, or exists only on a non-graph panel, shows one warning and renders no
marker for that title. These titles are Grafatui routing labels, not Grafana
panel IDs.

Events are ordered by timestamp. Unknown JSON fields are ignored. Times with
fractional seconds are accepted, and the full fractional timestamp is used when
projecting an event onto the graph even when the space-limited inline timestamp
display shows less precision.

## Target, Filter, Inspect, and Reload

This walkthrough uses current UTC timestamps so both events fall in the visible
15-minute range. It uses only POSIX shell tools; no `jq` is required.

First, start the bundled Prometheus demo stack from the repository root:

```bash
cd examples/demo && docker-compose up -d && sleep 5 && cd ../..
```

Then create the annotation file and run Grafatui:

```bash
annotation_demo_file=/tmp/grafatui-annotations-demo.jsonl
annotation_demo_time="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

printf '{"time":"%s","text":"Maintenance window","tags":["maintenance"]}\n' \
  "$annotation_demo_time" > "$annotation_demo_file"
printf '{"time":"%s","text":"API deployed","tags":["deploy","production"],"panel_titles":["HTTP Request Rate by Status Code"]}\n' \
  "$annotation_demo_time" >> "$annotation_demo_file"

cargo run -- \
  --grafana-json examples/dashboards/prometheus_demo.json \
  --prometheus-url http://localhost:19090 \
  --range 15m \
  --annotations-file "$annotation_demo_file"
```

`Maintenance window` appears on every graph/timeseries panel. `API deployed`
appears only on `HTTP Request Rate by Status Code`. Press `t`, select `deploy`
with `Space`, and press `Enter`; only the targeted deployment remains. Press
`v`, move the cursor to the marker, and press `Enter`; the selected panel's
cluster list and selected-event detail pane open.

While Grafatui is running, append an event in a second terminal:

```bash
annotation_demo_file=/tmp/grafatui-annotations-demo.jsonl
annotation_demo_time="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
printf '{"time":"%s","text":"Rollback started","tags":["rollback","production"],"panel_titles":["HTTP Request Rate by Status Code"]}\n' \
  "$annotation_demo_time" >> "$annotation_demo_file"
```

The new event is loaded after the normal refresh; Grafatui does not need to
restart. With only `deploy` selected, `Rollback started` remains hidden. Press
`t`, then `c`, and press `Enter` to apply the cleared filter and reveal the
rollback marker. Alternatively, select `rollback` in the filter and apply it.

## Tag Filter and Cluster Controls

Press `t` to open the global annotation tag filter. It is runtime-only: it is
not written to the JSONL source or configuration, and applies to every eligible
panel. Selected tags use exact, case-sensitive OR matching: an event remains
visible when it has any selected tag. With no selected tags, events with and
without tags remain visible. The catalogue keeps a selected tag with a zero
event count so it can be removed after a reload.

In the tag filter, use `Up`/`Down` or `j`/`k` to move, `Space` to toggle the
highlighted tag, `c` to clear the draft, `Enter` to apply it, or `Esc` to
cancel without changing the applied filter. In inspection mode, `Enter` opens
only the cluster actually rendered by the selected panel at the cursor. In the
cluster, use `Up`/`Down` or `j`/`k` to select an event, `PgUp`/`PgDn` to page,
and `Enter` or `Esc` to close. Mouse input is ignored while either annotation
modal is open.

Cluster contents are frozen when the cluster opens: a later reload can replace
the live annotation snapshot without moving rows or changing the cluster's
event details.

## Automatic Reload, Rendering, and Exports

Grafatui checks the file metadata during each normal refresh, including while
markers are hidden. When it changes, Grafatui reads, parses, and validates the
full candidate file before atomically replacing the snapshot. A zero-byte file
is a valid update that clears all events. Annotation loading is independent of
Prometheus: annotation failures never fail startup or a Prometheus refresh.

Only `graph` and `timeseries` panels receive annotation markers. Press `a` to
toggle marker visibility. Events that project to the same terminal column are
shown as one counted marker: `•` for one event, decimal `2`–`9` for two through
nine events, and `+` for 10 or more. In inspection mode, moving the cursor onto
that column shows the cluster's timestamp, text, and tags inline; multi-event
details report the exact cluster count.

Applied panel targeting and tag filtering affect the markers in SVG/PNG exports
and changed-frame recordings. Active inline annotation details are exportable;
the tag-filter and cluster modal chrome is not.

## Errors and Last Valid Snapshot

If the file is missing, unreadable, or contains a malformed event, Grafatui
keeps rendering the last valid snapshot and shows an annotation warning. A bad
update does not replace the previously loaded events, fail startup, or fail the
Prometheus refresh.

## Current Limits and Roadmap

This feature supports one external JSONL source, point events, panel-title
routing, and one global runtime-only tag filter. It has no stable event IDs, no
per-panel tag filters, no multiple sources, no editing, and no provider or API
work. It does not add Grafana annotation-query/API compatibility. Range events
and a provider API remain future roadmap work. `--validate` validates Grafana
dashboard imports only; it does not validate annotations.
