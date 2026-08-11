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
| `Up` / `Down` or `k` / `j` | Select previous or next panel |
| `PgUp` / `PgDn` | Scroll vertically, or select panels in fullscreen |
| `Home` / `End` | Jump to top or bottom |
| `y` | Toggle Y-axis mode |
| `g` | Toggle autogrid guide lines |
| `a` | Toggle external annotation markers |
| `t` | Open the global annotation tag filter |
| `1` through `9` | Toggle series visibility |
| `f` / `Enter` | Toggle fullscreen mode |
| `v` | Toggle value inspection mode |
| `Enter` in inspect mode | Open the selected panel's annotation cluster at the cursor |
| `e` | Export current view |
| `Ctrl+E` | Start or stop changed-frame recording |
| `/` | Search panels |
| `Left` / `Right` | Move cursor in inspect mode |
| `?` | Toggle debug info |

## Mouse Support

| Action | Behavior |
|---|---|
| Click | Select a panel, or move the cursor in fullscreen inspect mode |
| Drag | Move the cursor in fullscreen inspect mode |
| Scroll | Scroll the dashboard vertically |

In normal mode, clicking selects panels. Press `v` or `f` to use cursor-focused interactions.

## Annotation Modals

The global tag filter opens with `t`. Use `Up`/`Down` or `k`/`j` to move,
`Space` to toggle the highlighted tag, `c` to clear the draft, `Enter` to apply
it, or `Esc` to discard it. In an annotation cluster, use `Up`/`Down` or
`k`/`j` to select an event, `PgUp`/`PgDn` to page, and `Enter` or `Esc` to
close it. Mouse input is ignored while either annotation modal is open.
