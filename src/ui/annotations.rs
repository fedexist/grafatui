use super::layout::centered_rect;
use crate::annotations::{
    AnnotationModal, ClusterModalState, TagFilterModalState, format_event_time, visible_range,
};
use crate::app::AppState;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect, Size},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

const MODAL_WIDTH_PERCENT: u16 = 80;
const MODAL_HEIGHT_PERCENT: u16 = 70;

fn modal_area(size: Size) -> Rect {
    centered_rect(
        MODAL_WIDTH_PERCENT,
        MODAL_HEIGHT_PERCENT,
        Rect::new(0, 0, size.width, size.height),
    )
}

fn cluster_chunks(area: Rect) -> [Rect; 4] {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Percentage(45),
        Constraint::Min(2),
        Constraint::Length(1),
    ])
    .split(area);
    [chunks[0], chunks[1], chunks[2], chunks[3]]
}

pub(crate) fn annotation_cluster_page_size(size: Size) -> usize {
    let outer = modal_area(size);
    let inner = outer.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    usize::from(cluster_chunks(inner)[1].height).max(1)
}

pub(crate) fn render_annotation_modal(frame: &mut Frame, app: &AppState) {
    let Some(modal) = app.annotation_modal.as_ref() else {
        return;
    };
    let frame_area = frame.area();
    let area = modal_area(Size::new(frame_area.width, frame_area.height));
    frame.render_widget(Clear, area);
    match modal {
        AnnotationModal::Cluster(state) => render_cluster_modal(frame, area, state, app),
        AnnotationModal::TagFilter(state) => render_tag_filter_modal(frame, area, state, app),
    }
}

fn modal_block<'a>(title: &'a str, app: &AppState) -> Block<'a> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border_selected))
}

fn render_cluster_modal(frame: &mut Frame, area: Rect, state: &ClusterModalState, app: &AppState) {
    frame.render_widget(modal_block(" Annotation cluster ", app), area);
    let inner = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let [header_area, list_area, detail_area, hints_area] = cluster_chunks(inner);
    let total = state.events().len();
    frame.render_widget(
        Paragraph::new(format!(
            "{total} annotations    {} / {total}",
            state.selected().saturating_add(1)
        )),
        header_area,
    );

    let rows = usize::from(list_area.height);
    let visible = visible_range(total, state.selected(), rows);
    let items = state.events()[visible.clone()]
        .iter()
        .map(|event| {
            let time = if event.time.timestamp_subsec_millis() == 0 {
                event.time.format("%H:%M:%S").to_string()
            } else {
                event.time.format("%H:%M:%S%.3f").to_string()
            };
            ListItem::new(format!("{time}  {}", event.text))
        })
        .collect::<Vec<_>>();
    let mut list_state = ListState::default();
    if !items.is_empty() {
        list_state.select(Some(state.selected().saturating_sub(visible.start)));
    }
    let list = List::new(items).highlight_symbol("▶ ").highlight_style(
        Style::default()
            .fg(app.theme.title)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, list_area, &mut list_state);

    if let Some(event) = state.selected_event() {
        let tags = if event.tags.is_empty() {
            "Tags: —".to_string()
        } else {
            format!("Tags: {}", event.tags.join(", "))
        };
        let detail = Paragraph::new(vec![
            Line::from(format_event_time(event)),
            Line::from(event.text.as_str()),
            Line::from(tags),
        ])
        .wrap(Wrap { trim: false });
        frame.render_widget(detail, detail_area);
    }

    frame.render_widget(
        Paragraph::new("↑/↓ move  PgUp/PgDn page  Enter/Esc close"),
        hints_area,
    );
}

fn render_tag_filter_modal(
    frame: &mut Frame,
    area: Rect,
    state: &TagFilterModalState,
    app: &AppState,
) {
    frame.render_widget(modal_block(" Filter annotations by tag ", app), area);
    let inner = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);
    let list_area = chunks[0];
    let hints_area = chunks[1];
    let rows = usize::from(list_area.height);
    let visible = visible_range(state.entries().len(), state.selected(), rows);
    let row_width = usize::from(list_area.width);
    let items = state.entries()[visible.clone()]
        .iter()
        .map(|entry| {
            let checked = if state.draft().selected().contains(&entry.tag) {
                'x'
            } else {
                ' '
            };
            let label_width = row_width.saturating_sub(4 + entry.count.to_string().len());
            ListItem::new(format!(
                "[{checked}] {:<label_width$}{}",
                entry.tag, entry.count
            ))
        })
        .collect::<Vec<_>>();
    let mut list_state = ListState::default();
    if !items.is_empty() {
        list_state.select(Some(state.selected().saturating_sub(visible.start)));
    }
    let list = List::new(items).highlight_style(
        Style::default()
            .fg(app.theme.title)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, list_area, &mut list_state);
    frame.render_widget(
        Paragraph::new("Space toggle  c clear  Enter apply  Esc cancel"),
        hints_area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotations::{AnnotationEvent, AnnotationModal, AnnotationTarget, TagFilter};
    use crate::app::AppState;
    use crate::export::ExportOptions;
    use crate::prom::PromClient;
    use crate::theme::Theme;
    use ratatui::{Terminal, backend::TestBackend};
    use std::time::Duration;

    fn test_app() -> AppState {
        AppState::new(
            PromClient::new("http://localhost:9090".to_string()),
            Duration::from_secs(300),
            Duration::from_secs(5),
            Duration::from_secs(1),
            "Test".to_string(),
            vec![],
            0,
            Theme::default(),
            "dashed-line".to_string(),
            ExportOptions::default(),
        )
    }

    fn event(time: &str, text: &str, tags: &[&str]) -> AnnotationEvent {
        AnnotationEvent {
            time: chrono::DateTime::parse_from_rfc3339(time)
                .unwrap()
                .with_timezone(&chrono::Utc),
            text: text.to_string(),
            tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
            target: AnnotationTarget::All,
        }
    }

    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let mut output = String::new();
        for y in buffer.area.y..buffer.area.bottom() {
            for x in buffer.area.x..buffer.area.right() {
                output.push_str(buffer.cell((x, y)).unwrap().symbol());
            }
            output.push('\n');
        }
        output
    }

    #[test]
    fn cluster_modal_renders_summary_truncated_list_full_detail_and_hints() {
        let long_text = format!(
            "deploy {} wrapped detail reaches the final sentinel",
            "abcdefghijklmnopqrstuvwxyz".repeat(3)
        );
        let events = vec![
            event("2026-07-23T14:30:00.125Z", &long_text, &["prod", "api"]),
            event("2026-07-23T14:30:01Z", "rollback", &[]),
            event("2026-07-23T14:30:02Z", "resolved", &["incident"]),
        ];
        let mut app = test_app();
        app.annotation_modal = Some(AnnotationModal::Cluster(
            crate::annotations::ClusterModalState::new(events).unwrap(),
        ));
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();

        terminal
            .draw(|frame| render_annotation_modal(frame, &app))
            .unwrap();
        let text = buffer_text(&terminal);

        assert!(text.contains("3 annotations"));
        assert!(text.contains("1 / 3"));
        assert!(text.contains("2026-07-23 14:30:00.125 UTC"));
        assert!(text.contains("final sentinel"));
        assert!(text.contains("Tags: prod, api"));
        assert!(text.contains("↑/↓ move  PgUp/PgDn page  Enter/Esc close"));
        assert_eq!(text.matches("final sentinel").count(), 1);
    }

    #[test]
    fn tag_modal_renders_alphabetical_counts_selection_and_hints() {
        let mut app = test_app();
        app.annotations = crate::annotations::AnnotationState::from_events_for_test(vec![
            event("2026-07-23T14:30:00Z", "alert", &["incident"]),
            event("2026-07-23T14:31:00Z", "release", &["deploy"]),
            event("2026-07-23T14:32:00Z", "release two", &["deploy"]),
        ]);
        app.annotations
            .set_filter(TagFilter::from_selected(["deploy".to_string()]));
        app.open_tag_filter_modal();
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();

        terminal
            .draw(|frame| render_annotation_modal(frame, &app))
            .unwrap();
        let text = buffer_text(&terminal);
        let deploy = text.find("[x] deploy").unwrap();
        let incident = text.find("[ ] incident").unwrap();

        assert!(deploy < incident);
        assert!(
            text.lines()
                .any(|line| line.contains("│[x] deploy") && line.trim_end().ends_with("2│"))
        );
        assert!(
            text.lines()
                .any(|line| line.contains("│[ ] incident") && line.trim_end().ends_with("1│"))
        );
        assert!(text.contains("Space toggle  c clear  Enter apply  Esc cancel"));
    }

    #[test]
    fn cluster_page_size_matches_rendered_list_rows() {
        assert_eq!(annotation_cluster_page_size(Size::new(100, 40)), 12);
        assert!(annotation_cluster_page_size(Size::new(1, 1)) > 0);
    }

    #[test]
    fn annotation_modals_render_safely_on_a_small_terminal() {
        let mut app = test_app();
        app.annotation_modal = Some(AnnotationModal::Cluster(
            crate::annotations::ClusterModalState::new(vec![event(
                "2026-07-23T14:30:00Z",
                "deploy",
                &["prod"],
            )])
            .unwrap(),
        ));
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();

        terminal
            .draw(|frame| render_annotation_modal(frame, &app))
            .unwrap();

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer.area, Rect::new(0, 0, 40, 12));
        assert_eq!(buffer.content().len(), 40 * 12);
    }
}
