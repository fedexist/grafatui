# External Annotations

Grafatui can overlay read-only, external point events from one JSONL file. It
never edits or writes the source file.

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

The CLI option overrides the configured path. This source is opt-in and is
read-only; Grafatui does not create, edit, or otherwise write the file.

## JSONL Event Format

The file contains one JSON object per line. Blank lines are ignored. Each event
requires an RFC 3339 `time` and non-empty `text`; `tags` is optional, but every
provided tag must be a non-empty string.

```json
{"time":"2026-07-23T14:30:00Z","text":"Deployed v2.4","tags":["deploy","production"]}
```

Events are ordered by timestamp. Unknown JSON fields are ignored. Times with
fractional seconds are accepted, and the full fractional timestamp is used when
projecting an event onto the graph even when the space-limited inline timestamp
display shows less precision.

## Automatic Reload

Grafatui checks the file during each normal refresh and reloads it when its
metadata changes. You can therefore update the JSONL file while Grafatui is
running without restarting it. Annotation loading is independent of Prometheus:
annotation failures never fail startup or a Prometheus refresh.

## Rendering and Inspection

Only `graph` and `timeseries` panels receive annotation markers. Press `a` to
toggle external annotation marker visibility. Events that project to the same
terminal column are shown as one counted marker; in inspection mode, moving the
cursor onto that column shows the clustered events' timestamp, text, and tags
inline.

Visible markers and active inline annotation details are included in SVG/PNG
exports and changed-frame recordings.

## Errors and Last Valid Snapshot

If the file is missing, unreadable, or contains a malformed event, Grafatui
keeps rendering the last valid snapshot and shows an annotation warning. A bad
update does not replace the previously loaded events, fail startup, or fail the
Prometheus refresh.

## Current Limitations

This iteration supports one external JSONL source and point events applied to
all graph/timeseries panels. It does not support panel targeting, a navigable
annotation popup, or tag filtering. `--validate` validates Grafana dashboard
imports only; it does not validate annotations.

External JSONL events are separate from Grafana dashboard `annotations` and
`annotations.list`, which remain unsupported. Grafatui also does not run
Prometheus annotation queries or implement Grafana annotation-query/API
compatibility.

## Roadmap

Iteration 1 is the shipped external JSONL point-annotation foundation. A future
iteration 2 PR will add a navigable popup, panel targeting, and tag filtering.
A separate future iteration 3 PR will introduce a provider API. Neither future
iteration is implemented yet.
