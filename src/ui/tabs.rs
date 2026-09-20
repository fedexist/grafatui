use crate::theme::Theme;
use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TabSegment {
    pub(crate) rect: Rect,
    pub(crate) text: String,
    pub(crate) active: bool,
    pub(crate) activate: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TabBarGeometry {
    pub(crate) segments: Vec<TabSegment>,
}

pub(crate) fn tab_title(title: &str, index: usize) -> String {
    let sanitized = title
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    if sanitized.trim().is_empty() {
        format!("Tab {}", index + 1)
    } else {
        sanitized
    }
}

pub(crate) fn truncate_title(title: &str, width: u16) -> String {
    if width == 0 {
        return String::new();
    }
    let span = Span::raw(title);
    if span.width() <= usize::from(width) {
        return title.to_owned();
    }
    let budget = usize::from(width) - 1;
    let mut text = String::new();
    let mut used = 0;
    for grapheme in span.styled_graphemes(Style::default()) {
        let size = Span::raw(grapheme.symbol).width();
        if used + size > budget {
            break;
        }
        text.push_str(grapheme.symbol);
        used += size;
    }
    text.push('…');
    text
}

fn text_width(text: &str) -> usize {
    Span::raw(text).width()
}

fn range_width(labels: &[String], active: usize, start: usize, end: usize) -> usize {
    (start..=end)
        .map(|index| text_width(&labels[index]) + usize::from(index == active) * 2)
        .sum::<usize>()
        + end.saturating_sub(start)
        + usize::from(start > 0) * 2
        + usize::from(end + 1 < labels.len()) * 2
}

pub(crate) fn tab_bar_geometry(
    area: Rect,
    titles: &[String],
    active: Option<usize>,
    depth: usize,
) -> TabBarGeometry {
    if area.is_empty() {
        return TabBarGeometry::default();
    }
    let mut geometry = TabBarGeometry::default();
    let indent = (depth.saturating_mul(2)).min(usize::from(area.width.saturating_sub(1)));
    let mut x = area.x.saturating_add(indent as u16);
    let available = usize::from(area.right().saturating_sub(x));
    if available == 0 {
        return geometry;
    }
    if titles.is_empty() || active.is_none() {
        let text = truncate_title("No tabs", available as u16);
        geometry.segments.push(TabSegment {
            rect: Rect::new(x, area.y, text_width(&text) as u16, 1),
            text,
            active: false,
            activate: None,
        });
        return geometry;
    }

    let labels = titles
        .iter()
        .enumerate()
        .map(|(index, title)| tab_title(title, index))
        .collect::<Vec<_>>();
    let active = active.unwrap().min(labels.len() - 1);
    let mut start = active;
    let mut end = active;
    if area.width >= 3 {
        loop {
            let mut grew = false;
            if start > 0 && range_width(&labels, active, start - 1, end) <= available {
                start -= 1;
                grew = true;
            }
            if end + 1 < labels.len() && range_width(&labels, active, start, end + 1) <= available {
                end += 1;
                grew = true;
            }
            if !grew {
                break;
            }
        }
    }

    let mut push = |text: String, selected: bool, activate: Option<usize>| {
        let remaining = area.right().saturating_sub(x);
        if remaining == 0 {
            return;
        }
        let text = truncate_title(&text, remaining);
        let width = text_width(&text).min(usize::from(remaining)) as u16;
        if width == 0 {
            return;
        }
        geometry.segments.push(TabSegment {
            rect: Rect::new(x, area.y, width, 1),
            text,
            active: selected,
            activate,
        });
        x = x.saturating_add(width);
    };

    if area.width < 3 {
        push(format!("* {}", labels[active]), true, Some(active));
        return geometry;
    }
    if start > 0 {
        push("< ".to_string(), false, Some(active - 1));
    }
    for (index, label) in labels.iter().enumerate().take(end + 1).skip(start) {
        if index > start {
            push(" ".to_string(), false, None);
        }
        let label = if index == active {
            format!("* {label}")
        } else {
            label.clone()
        };
        push(label, index == active, Some(index));
    }
    if end + 1 < labels.len() {
        push(" >".to_string(), false, Some(active + 1));
    }
    geometry
}

pub(crate) fn tab_at(geometry: &TabBarGeometry, position: Position) -> Option<usize> {
    geometry
        .segments
        .iter()
        .find(|segment| segment.rect.contains(position))
        .and_then(|segment| segment.activate)
}

pub(crate) fn tab_style(theme: &Theme, active: bool, focused: bool) -> Style {
    let mut style = Style::default().fg(if active { theme.title } else { theme.text });
    if active {
        style = style.add_modifier(Modifier::BOLD);
    }
    if focused {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

pub(crate) fn render_tab_bar(
    frame: &mut Frame,
    geometry: &TabBarGeometry,
    theme: &Theme,
    focused: bool,
) {
    for segment in &geometry.segments {
        frame.render_widget(
            Line::styled(
                segment.text.as_str(),
                tab_style(theme, segment.active, focused),
            ),
            segment.rect,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{layout::Rect, text::Span};

    #[test]
    fn tabs_overflow_keeps_active_label_clickable() {
        let titles = vec!["Overview".into(), "服务".into(), "Latency".into()];
        let area = Rect::new(2, 3, 12, 1);
        let geometry = tab_bar_geometry(area, &titles, Some(2), 0);
        let active = geometry.segments.iter().find(|part| part.active).unwrap();
        assert_eq!(active.activate, Some(2));
        assert!(!active.text.is_empty());
        assert!(Span::raw(&active.text).width() <= usize::from(active.rect.width));
        assert_eq!(tab_at(&geometry, active.rect.as_position()), Some(2));
        let left_overflow = geometry.segments.first().unwrap();
        assert_eq!(left_overflow.text, "< ");
        assert_eq!(left_overflow.activate, Some(1));
        assert_eq!(tab_at(&geometry, left_overflow.rect.as_position()), Some(1));
        assert!(
            geometry
                .segments
                .iter()
                .all(|part| part.rect.x >= area.x && part.rect.right() <= area.right())
        );
    }

    #[test]
    fn tabs_truncation_preserves_clusters() {
        assert_eq!(truncate_title("服务延迟", 5), "服务…");
        assert_eq!(truncate_title("e\u{301}xyz", 2), "e\u{301}…");
        assert_eq!(truncate_title("Latency", 0), "");
        assert_eq!(tab_title("  ", 2), "Tab 3");
    }
}
