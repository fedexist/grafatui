/*
 * Copyright 2025 Federico D'Ambrosio
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use super::labels::PlotBounds;
use ratatui::prelude::*;

pub(super) fn merge_overlay_buffer(
    frame: &mut Frame,
    overlay_buf: &ratatui::buffer::Buffer,
    plot: PlotBounds,
) {
    let buf = frame.buffer_mut();
    for y in plot.top..=plot.bottom {
        for x in plot.left..plot.right {
            let Some(src_cell) = overlay_buf.cell((x, y)) else {
                continue;
            };
            if let Some(dst_cell) = buf.cell_mut((x, y)) {
                overlay_cell_if_blank(dst_cell, src_cell);
            }
        }
    }
}

pub(super) fn is_blank_cell(cell: &ratatui::buffer::Cell) -> bool {
    cell.symbol().chars().all(char::is_whitespace)
}

fn overlay_cell_if_blank(dst: &mut ratatui::buffer::Cell, src: &ratatui::buffer::Cell) {
    if is_blank_cell(dst) && !is_blank_cell(src) {
        dst.set_symbol(src.symbol()).set_style(src.style());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Style;

    #[test]
    fn test_is_blank_cell_empty() {
        let cell = ratatui::buffer::Cell::default();
        assert!(is_blank_cell(&cell));
    }

    #[test]
    fn test_is_blank_cell_filled() {
        let mut cell = ratatui::buffer::Cell::default();
        cell.set_char('x');
        assert!(!is_blank_cell(&cell));
    }

    #[test]
    fn test_overlay_cell_if_blank_copies_when_destination_is_empty() {
        let mut dst = ratatui::buffer::Cell::default();
        let mut src = ratatui::buffer::Cell::default();
        src.set_char('-').set_style(Style::default().fg(Color::Red));

        overlay_cell_if_blank(&mut dst, &src);

        assert_eq!(dst.symbol(), "-");
        assert_eq!(dst.style().fg, Some(Color::Red));
    }

    #[test]
    fn test_overlay_cell_if_blank_keeps_existing_destination_marker() {
        let mut dst = ratatui::buffer::Cell::default();
        dst.set_char('x')
            .set_style(Style::default().fg(Color::LightBlue));
        let mut src = ratatui::buffer::Cell::default();
        src.set_char('-').set_style(Style::default().fg(Color::Red));

        overlay_cell_if_blank(&mut dst, &src);

        assert_eq!(dst.symbol(), "x");
        assert_eq!(dst.style().fg, Some(Color::LightBlue));
    }
}
