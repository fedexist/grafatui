//! Plain-text wrapping shared by fitting error measurement and visible windows.
// Word-boundary handling is adapted from Ratatui 0.30.2 (MIT), reflow::WordWrapper.
/*
The MIT License (MIT)

Copyright (c) 2016-2022 Florian Dehau
Copyright (c) 2023-2025 The Ratatui Developers

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/
// Streaming emission avoids retaining the entire reflow of a long physical line.
use ratatui::{buffer::CellWidth, style::Style, text::Text};
use std::collections::VecDeque;

pub(crate) struct ErrorBodyWindow {
    pub(crate) total_lines: u64,
    pub(crate) first_line: u64,
    pub(crate) lines: Vec<String>,
}

pub(crate) fn error_body_window(
    text: &str,
    width: u16,
    offset: u64,
    capacity: u16,
) -> ErrorBodyWindow {
    let mut total = 0_u64;
    visit_wrapped_lines(text, width, |_| total = total.saturating_add(1));
    let first = offset.min(total.saturating_sub(u64::from(capacity)));
    let mut lines = Vec::new();
    if capacity > 0 {
        let mut row = 0_u64;
        visit_wrapped_lines(text, width, |line| {
            if row >= first && lines.len() < usize::from(capacity) {
                lines.push(line.to_owned());
            }
            row = row.saturating_add(1);
        });
    }
    ErrorBodyWindow {
        total_lines: total,
        first_line: first,
        lines,
    }
}

fn visit_wrapped_lines(text: &str, width: u16, mut emit: impl FnMut(&str)) {
    if width == 0 {
        return;
    }
    let limit = u32::from(width);
    for line in Text::from(text).lines {
        let mut pending_line: Vec<&str> = Vec::new();
        let mut pending_word: Vec<&str> = Vec::new();
        let mut whitespace: VecDeque<&str> = VecDeque::new();
        let (mut line_width, mut word_width, mut whitespace_width) = (0_u32, 0_u32, 0_u32);
        let mut previous_nonspace = false;
        let mut emitted = false;
        for grapheme in line.styled_graphemes(Style::default()) {
            let space = grapheme.is_whitespace();
            let size = u32::from(grapheme.symbol.cell_width());
            if size > limit {
                continue;
            }
            let word_found = previous_nonspace && space;
            let overflow = pending_line.is_empty()
                && (word_width + size > limit || whitespace_width + size > limit);
            if word_found || overflow {
                if !pending_line.is_empty() {
                    pending_line.extend(whitespace.drain(..));
                    line_width += whitespace_width;
                }
                pending_line.append(&mut pending_word);
                line_width += word_width;
                whitespace.clear();
                whitespace_width = 0;
                word_width = 0;
            }
            if line_width >= limit
                || (size > 0 && line_width + whitespace_width + word_width >= limit)
            {
                let mut remaining = limit.saturating_sub(line_width);
                emit(&pending_line.concat());
                emitted = true;
                pending_line.clear();
                line_width = 0;
                while let Some(symbol) = whitespace.front() {
                    let size = u32::from(symbol.cell_width());
                    if size > remaining {
                        break;
                    }
                    whitespace_width -= size;
                    remaining -= size;
                    whitespace.pop_front();
                }
                if space && whitespace.is_empty() {
                    continue;
                }
            }
            if space {
                whitespace_width += size;
                whitespace.push_back(grapheme.symbol);
            } else {
                word_width += size;
                pending_word.push(grapheme.symbol);
            }
            previous_nonspace = !space;
        }
        if pending_line.is_empty() && pending_word.is_empty() && !whitespace.is_empty() {
            emit("");
            emitted = true;
        }
        if !pending_line.is_empty() {
            pending_line.extend(whitespace.drain(..));
        }
        pending_line.append(&mut pending_word);
        if !pending_line.is_empty() {
            emit(&pending_line.concat());
        } else if !emitted {
            emit("");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{
        buffer::Buffer,
        layout::Rect,
        widgets::{Paragraph, Widget, Wrap},
    };

    #[test]
    fn bounds_scroll_error_windows_match_literal_wraps() {
        let window = error_body_window("one two three four", 8, 0, 10);
        assert_eq!(window.total_lines, 3);
        assert_eq!(window.first_line, 0);
        assert_eq!(window.lines, ["one two", "three", "four"]);
        let end = error_body_window("one two three four", 8, u64::MAX, 2);
        assert_eq!(end.first_line, 1);
        assert_eq!(end.lines, ["three", "four"]);
        assert_eq!(error_body_window("", 8, 0, 3).lines, [""]);
        assert_eq!(error_body_window("x", 0, 0, 3).total_lines, 0);
    }

    // A divergent wrapper loses content or changes cell positions in the real renderer.
    #[test]
    fn bounds_scroll_error_wrap_matches_ratatui_buffers() {
        for text in [
            "",
            " ",
            "  one two  three four",
            "one\ntwo\n\nthree\n",
            "\n",
            "a\tb c",
            "abcdefghijklmnopqrstuvwxyz",
            "水水 é e\u{301} x",
            "a\u{200b}b c",
            "a\u{a0}b c",
            "a\r\nb",
            " x ",
            "👩‍💻 café 👨‍👩‍👧",
        ] {
            for width in [1, 2, 8, 18, 40] {
                let rect = Rect::new(0, 0, width, 40);
                let mut expected = Buffer::empty(rect);
                Paragraph::new(text)
                    .wrap(Wrap { trim: true })
                    .render(rect, &mut expected);
                let w = error_body_window(text, width, 0, 40);
                let mut actual = Buffer::empty(rect);
                let lines = w
                    .lines
                    .into_iter()
                    .map(ratatui::text::Line::from)
                    .collect::<Vec<_>>();
                Paragraph::new(lines).render(rect, &mut actual);
                assert_eq!(actual, expected, "{text:?} at width{width}");
            }
        }
    }

    #[test]
    fn bounds_scroll_error_windows_are_wide_and_unicode_safe() {
        let text = (0..70_000)
            .map(|n| format!("line{n:05}"))
            .collect::<Vec<_>>()
            .join("\n");
        let w = error_body_window(&text, 40, u64::MAX, 8);
        assert_eq!(
            (w.total_lines, w.first_line, w.lines.len()),
            (70_000, 69_992, 8)
        );
        assert_eq!(w.lines.last().unwrap(), "line69999");
        let text = "a".repeat(70_000);
        let w = error_body_window(&text, 1, u64::MAX, 3);
        assert_eq!(
            (w.total_lines, w.first_line, w.lines.len()),
            (70_000, 69_997, 3)
        );
        assert_eq!(w.lines, ["a", "a", "a"]);
        let count = error_body_window(&text, 1, 0, 0);
        assert_eq!(count.total_lines, 70_000);
        assert!(count.lines.is_empty());
    }
}
