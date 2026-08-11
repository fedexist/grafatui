# Ratatui 0.30 buffer capture harness

Use this harness for deterministic, structural visual evidence. It converts a
Ratatui `TestBackend` buffer to the version-1 JSON input accepted by
`scripts/render-buffer`; it is not a replacement for behavioral assertions or
real-terminal smoke coverage.

## Determinism prerequisites

Before constructing the state, fix every input that can change a frame:

- time and timezone;
- fixture data, ordering, IDs, and network responses;
- viewport dimensions;
- selected, focused, scroll, overlay, and mode state; and
- the complete theme, including the colors used for `Color::Reset`.

Use named, checked-in fixtures that make the intended condition obvious. Do
not obtain data from a live service, system clock, random source, locale, or
terminal during a capture. A capture should be byte-stable when re-run with
the same fixture.

## Render with TestBackend

Create `Terminal<TestBackend>` at the scenario's fixed dimensions and render
one fully prepared `AppState`. Capture only after `Terminal::draw` returns:
that is when Ratatui has applied the frame to the backend. Do not mutate the
backend directly.

## Assert behavior before appearance

First exercise the app's reducer, command, or event handler and assert the
expected state transition exactly. Then render that asserted state and compare
the resulting buffer. A visually plausible frame cannot prove that the right
selection, query, mode, or data was produced.

For example, a test should establish `selected_row == 3` and
`overlay == Overlay::Help` before it captures the focused help frame. Keep a
state assertion and a focused buffer assertion even when a rendered PNG is
also retained.

## Normalized capture schema

`render-buffer` accepts this JSON object. All colors are lowercase `#rrggbb`.
Cells are exhaustive and ordered row-major; modifiers appear in this fixed
order when present: `bold`, `dim`, `italic`, `underlined`, `reversed`,
`crossed_out`.

```json
{
  "version": 1,
  "width": 80,
  "height": 24,
  "cell_width": 8,
  "cell_height": 16,
  "default_fg": "#d0d0d0",
  "default_bg": "#101010",
  "cells": [
    {
      "x": 0,
      "y": 0,
      "symbol": "G",
      "fg": "#d0d0d0",
      "bg": "#101010",
      "modifiers": ["bold"]
    }
  ],
  "cursor": { "x": 0, "y": 0 }
}
```

`cursor` is `null` when hidden; otherwise it is the visible backend cursor
position. Choose one fixed cell size, font, and renderer configuration for a
baseline family. Create an SVG (and, where Chromium is available, PNG) from a
capture with:

```bash
.agents/skills/testing-ratatui-tuis/scripts/render-buffer \
  --input artifacts/capture.json \
  --svg artifacts/capture.svg \
  --png artifacts/capture.png
```

## Complete Rust capture example

This self-contained Ratatui 0.30.1 example has a deliberately small
`AppState`; replace its `render` implementation with the application render
entry point while preserving the capture code. It needs `serde` with `derive`
and `serde_json` in addition to `ratatui`.

```rust
use std::{error::Error, io::Write};

use ratatui::{
    backend::TestBackend,
    style::{Color, Modifier, Style},
    widgets::Paragraph,
    Frame, Terminal,
};
use serde::Serialize;

const DEFAULT_FG: &str = "#d0d0d0";
const DEFAULT_BG: &str = "#101010";

struct AppState {
    title: String,
    focused: bool,
}

impl AppState {
    fn render(&self, frame: &mut Frame) {
        let style = if self.focused {
            Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        frame.render_widget(Paragraph::new(self.title.as_str()).style(style), frame.area());
        if self.focused {
            frame.set_cursor_position((0, 0));
        }
    }
}

#[derive(Serialize)]
struct Capture {
    version: u8,
    width: u16,
    height: u16,
    cell_width: u16,
    cell_height: u16,
    default_fg: &'static str,
    default_bg: &'static str,
    cells: Vec<CapturedCell>,
    cursor: Option<Cursor>,
}

#[derive(Serialize)]
struct CapturedCell {
    x: u16,
    y: u16,
    symbol: String,
    fg: String,
    bg: String,
    modifiers: Vec<&'static str>,
}

#[derive(Serialize)]
struct Cursor {
    x: u16,
    y: u16,
}

fn color_to_hex(color: Color, reset: &str) -> String {
    let (red, green, blue) = match color {
        Color::Reset => return reset.to_owned(),
        Color::Black => (0, 0, 0),
        Color::Red => (128, 0, 0),
        Color::Green => (0, 128, 0),
        Color::Yellow => (128, 128, 0),
        Color::Blue => (0, 0, 128),
        Color::Magenta => (128, 0, 128),
        Color::Cyan => (0, 128, 128),
        Color::Gray => (192, 192, 192),
        Color::DarkGray => (128, 128, 128),
        Color::LightRed => (255, 0, 0),
        Color::LightGreen => (0, 255, 0),
        Color::LightYellow => (255, 255, 0),
        Color::LightBlue => (0, 0, 255),
        Color::LightMagenta => (255, 0, 255),
        Color::LightCyan => (0, 255, 255),
        Color::White => (255, 255, 255),
        Color::Rgb(red, green, blue) => (red, green, blue),
        Color::Indexed(index) => indexed_to_rgb(index),
    };
    format!("#{red:02x}{green:02x}{blue:02x}")
}

fn indexed_to_rgb(index: u8) -> (u8, u8, u8) {
    const ANSI: [(u8, u8, u8); 16] = [
        (0, 0, 0), (128, 0, 0), (0, 128, 0), (128, 128, 0),
        (0, 0, 128), (128, 0, 128), (0, 128, 128), (192, 192, 192),
        (128, 128, 128), (255, 0, 0), (0, 255, 0), (255, 255, 0),
        (0, 0, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255),
    ];
    match index {
        0..=15 => ANSI[index as usize],
        16..=231 => {
            const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
            let cube = index - 16;
            (
                LEVELS[(cube / 36) as usize],
                LEVELS[((cube / 6) % 6) as usize],
                LEVELS[(cube % 6) as usize],
            )
        }
        232..=255 => {
            let gray = 8 + (index - 232) * 10;
            (gray, gray, gray)
        }
    }
}

fn modifiers(modifier: Modifier) -> Vec<&'static str> {
    [
        (Modifier::BOLD, "bold"),
        (Modifier::DIM, "dim"),
        (Modifier::ITALIC, "italic"),
        (Modifier::UNDERLINED, "underlined"),
        (Modifier::REVERSED, "reversed"),
        (Modifier::CROSSED_OUT, "crossed_out"),
    ]
    .into_iter()
    .filter_map(|(flag, name)| modifier.contains(flag).then_some(name))
    .collect()
}

fn capture(app: &AppState, width: u16, height: u16) -> Result<Capture, Box<dyn Error>> {
    let backend = TestBackend::new(width, height);
    let mut terminal: Terminal<TestBackend> = Terminal::new(backend)?;
    terminal.draw(|frame| app.render(frame))?;

    let buffer = terminal.backend().buffer();
    let mut cells = Vec::with_capacity(width as usize * height as usize);
    for y in 0..height {
        for x in 0..width {
            let cell = buffer.cell((x, y)).expect("TestBackend cell is in bounds");
            cells.push(CapturedCell {
                x,
                y,
                symbol: cell.symbol().to_owned(),
                fg: color_to_hex(cell.fg, DEFAULT_FG),
                bg: color_to_hex(cell.bg, DEFAULT_BG),
                modifiers: modifiers(cell.modifier),
            });
        }
    }

    let backend = terminal.backend();
    let position = backend.cursor_position();
    let cursor = backend.cursor_visible()
        && position.x < width
        && position.y < height;
    Ok(Capture {
        version: 1,
        width,
        height,
        cell_width: 8,
        cell_height: 16,
        default_fg: DEFAULT_FG,
        default_bg: DEFAULT_BG,
        cells,
        cursor: cursor.then_some(Cursor { x: position.x, y: position.y }),
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let app = AppState { title: "Grafatui".to_owned(), focused: true };
    let capture = capture(&app, 80, 24)?;
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer_pretty(&mut output, &capture)?;
    writeln!(output)?;
    Ok(())
}
```

The conversion deliberately maps every Ratatui `Color` variant. `Reset` is
converted to the fixed foreground or background default passed by the caller;
`Rgb` is emitted directly; indexed `16..=231` uses the xterm 6x6x6 color cube,
and `232..=255` uses its grayscale ramp. Indexed `0..=15` uses the same ANSI
palette as the named variants. Unsupported display modifiers are intentionally
not serialized because the shared renderer schema accepts only the six listed
above; assert them separately if they matter to behavior.

### API verification

Grafatui's `ratatui = 0.30.1` resolves to `ratatui-core 0.1.1`. The capture
pattern was checked against the installed source with:

```bash
rg -n 'pub const fn buffer|pub fn symbol|pub fg:|pub bg:|pub modifier:' \
  "$HOME/.cargo/registry/src"/index.crates.io-*/ratatui-core-0.1.1/src
```

It uses the verified `TestBackend::buffer()`, `Buffer::cell((x, y))`,
`Cell::symbol()`, and public `Cell::fg`, `Cell::bg`, and `Cell::modifier`
fields.

## Viewport selection

Use the generated scenario matrix to select viewports. The default is the
representative normal viewport. Add a narrow or short viewport only when the
scenario says it exercises a relevant threshold, clipping risk, layout branch,
or interaction path. Record the exact columns x rows with every artifact;
never let the host terminal choose it.

## Live-service boundary

The TestBackend capture is a unit/integration harness, not evidence that the
interactive binary is terminal-safe. Keep services behind deterministic
fixtures here. Validate terminal initialization, alternate-screen behavior,
keypress wiring, resize handling, and shutdown separately through
`scripts/pty-smoke` and an explicitly bounded PTY scenario. Mark unavailable
external capabilities as inconclusive rather than fabricating visual evidence.
