# Keyboard and Mouse

Grafatui is designed for keyboard-first dashboard inspection.

## Keyboard Controls

| Key | Action |
|---|---|
| `q` | Quit |
| `r` | Force refresh |
| `+` / `-` | Zoom out / in |
| `[` / `]` | Pan left / right in time |
| `0` | Reset to live mode |
| `Up` / `Down` or `k` / `j` | Select previous or next visible row or panel |
| `Enter` / `Space` | Toggle the selected row |
| `Left` / `Right` | Collapse / expand the selected row |
| `PgUp` / `PgDn` | Scroll vertically, or select panels in fullscreen |
| `Home` / `End` | Jump to top or bottom |
| `Ctrl+Up` / `Ctrl+Down` | Scroll selected capped AutoGrid table/error body by one row |
| `Ctrl+PgUp` / `Ctrl+PgDn` | Page that body by its visible capacity |
| `Ctrl+Home` / `Ctrl+End` | First / last body window |
| `y` | Toggle Y-axis mode |
| `g` | Toggle autogrid guide lines |
| `a` | Toggle external annotation markers |
| `t` | Open the global annotation tag filter |
| `1` through `9` | Toggle series visibility |
| `f` | Toggle fullscreen mode for the selected panel |
| `v` | Toggle value inspection mode |
| `Enter` in inspect mode | Open the selected panel's annotation cluster at the cursor |
| `e` | Export current view |
| `Ctrl+E` | Start or stop changed-frame recording |
| `/` | Search visible rows and panels |
| `Left` / `Right` | Move cursor in inspect mode |
| `?` | Toggle debug info |

## Mouse Support

| Action | Behavior |
|---|---|
| Click | Select a row or panel; click a row disclosure marker to toggle it; move the cursor in fullscreen inspect mode |
| Drag | Move the cursor in fullscreen inspect mode |
| Scroll | Scroll the selected overflowing capped table/error body under the pointer; otherwise scroll the dashboard |
| Shift+scroll | Scroll the dashboard vertically |

In normal mode, clicking selects rows or panels. Press `v` or `f` on a selected
panel to use cursor-focused interactions.

## Annotation Modals

The global tag filter opens with `t`. Use `Up`/`Down` or `k`/`j` to move,
`Space` to toggle the highlighted tag, `c` to clear the draft, `Enter` to apply
it, or `Esc` to discard it. In an annotation cluster, use `Up`/`Down` or
`k`/`j` to select an event, `PgUp`/`PgDn` to page, and `Enter` or `Esc` to
close it. Mouse input is ignored while either annotation modal is open.

Body controls require a fitting Table in an AutoGrid group with a finite maximum.
They use exactly Ctrl, outside search and annotation modals; borders and table
headers stay fixed. The footer shows a hint when that body can scroll. Wheel
input over headers, borders, gaps or other panels scrolls the dashboard; wheel
input at an owned body's boundary stays with that body. Unmodified navigation
keys keep their existing actions. Press `f` when a tiny cap leaves no visible
body; fullscreen bypasses the cap and retains these controls.
