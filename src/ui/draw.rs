/*
 * Copyright 2026 Federico D'Ambrosio
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

use super::layout::{calculate_grid_layout, calculate_two_column_layout, centered_rect};
use super::panels::render_panel;
use crate::app::{AppMode, AppState, PanelState};
use humantime::format_duration;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

pub(crate) fn draw_ui(frame: &mut Frame, app: &mut AppState) {
    let size = frame.area();

    // Layout: title bar, charts area, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(size);

    // Title
    let title_text = format!(
        "{} — range={} step={}  panels={}  {}(r to refresh, +/- range, [] pan, 0 live, q quit)",
        app.title,
        format_duration(app.range),
        format_duration(app.step),
        app.panels.len(),
        if app.is_live() { "" } else { "⏸ PAUSED " }
    );
    let title_block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(title_text).alignment(Alignment::Center));
    frame.render_widget(title_block, chunks[0]);

    // Charts area: use Grafana grid if any panel has it, else fallback to 2-column flow
    let area = chunks[1];
    let charts_block = Block::default().borders(Borders::ALL);
    frame.render_widget(charts_block, area);
    let inner_area = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    let mut selected_rendered_cluster = None;
    if app.mode == AppMode::Fullscreen || app.mode == AppMode::FullscreenInspect {
        if let Some(p) = app.panels.get(app.selected_panel) {
            selected_rendered_cluster = render_panel(
                frame,
                inner_area,
                app.selected_panel,
                p,
                app,
                true,
                app.cursor_x,
            );
        }
    } else {
        let has_grid = app.panels.iter().any(|p| p.grid.is_some());

        let panel_rects = if has_grid {
            calculate_grid_layout(inner_area, app)
        } else {
            calculate_two_column_layout(inner_area, app)
        };

        for (rect, panel_idx) in &panel_rects {
            // eprintln!("Rendering panel {} at {:?}", panel_idx, rect);
            if let Some(p) = app.panels.get(*panel_idx) {
                let is_selected = *panel_idx == app.selected_panel;
                let rendered_cluster =
                    render_panel(frame, *rect, *panel_idx, p, app, is_selected, app.cursor_x);
                if is_selected {
                    selected_rendered_cluster = rendered_cluster;
                }
            }
        }

        if !has_grid && app.panels.is_empty() {
            // No panels to render
        } else if has_grid {
            // Check if we need to render extras (panels without grid)
            // The calculate_grid_layout should handle extras too?
            // The original code handled extras by stacking them below.
            // Let's make calculate_grid_layout return extras too.
        }
    }
    app.rendered_annotation_cluster = selected_rendered_cluster;

    // Footer / Status bar
    let errors = app.panels.iter().filter(|p| p.last_error.is_some()).count();
    let panel_count_display =
        if app.mode == AppMode::Fullscreen || app.mode == AppMode::FullscreenInspect {
            "1 (Fullscreen)".to_string()
        } else {
            format!("{}", app.panels.len())
        };

    let mode_display = match app.mode {
        AppMode::Normal => "NORMAL",
        AppMode::Search => "SEARCH",
        AppMode::Fullscreen => "FULLSCREEN",
        AppMode::Inspect => "INSPECT",
        AppMode::FullscreenInspect => "FULLSCREEN INSPECT",
    };

    let summary = format!(
        "Mode: {}{} | Prom: {} | range={} step={:?} refresh={} | grid={} | panels={} (skipped {}) errors={} | keys: ↑/↓ scroll, r refresh, e export, Ctrl+E record, +/- range, q quit, ? debug:{}",
        mode_display,
        if app.recording.is_some() { " REC" } else { "" },
        app.prometheus.base,
        format_duration(app.range),
        app.step,
        format_duration(app.refresh_every),
        if app.autogrid_enabled { "on" } else { "off" },
        panel_count_display,
        app.skipped_panels,
        errors,
        if app.debug_bar { "on" } else { "off" }
    );

    let detail = build_footer_detail(app);

    let footer = Paragraph::new(format!("{}\n{}", summary, detail)).wrap(Wrap { trim: true });
    frame.render_widget(footer, chunks[2]);

    // Search Popup
    if app.mode == AppMode::Search {
        let area = centered_rect(60, 20, size);
        let block = Block::default()
            .title(" Search Panels ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border_selected));
        frame.render_widget(Clear, area); // Clear background
        frame.render_widget(block, area);

        let inner_area = area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        });
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner_area);

        // Input
        let input = Paragraph::new(format!("> {}", app.search_query))
            .style(Style::default().fg(app.theme.text));
        frame.render_widget(input, chunks[0]);

        // Results
        let results: Vec<ListItem> = app
            .search_results
            .iter()
            .map(|&idx| {
                let p = &app.panels[idx];
                ListItem::new(format!("• {}", p.title))
            })
            .collect();
        let list = List::new(results)
            .block(Block::default().borders(Borders::TOP))
            .highlight_style(
                Style::default()
                    .fg(app.theme.title)
                    .add_modifier(Modifier::BOLD)
                    .bg(app.theme.background), // Optional: add background to make it pop more?
            )
            .highlight_symbol(">> ");

        let mut list_state = ratatui::widgets::ListState::default();
        if !app.search_results.is_empty() {
            list_state.select(Some(0));
        }
        frame.render_stateful_widget(list, chunks[1], &mut list_state);
    }

    super::render_annotation_modal(frame, app);
}

fn build_footer_detail(app: &AppState) -> String {
    let mut parts = Vec::new();

    if matches!(app.mode, AppMode::Inspect | AppMode::FullscreenInspect)
        && let Some(cx) = app.cursor_x
    {
        let cursor_time = chrono::DateTime::from_timestamp(cx as i64, 0)
            .map(|dt| dt.format("%H:%M:%S").to_string())
            .unwrap_or_default();
        parts.push(format!("Cursor: {cursor_time}"));
    }

    if let Some(status) = app.annotations.footer_status() {
        parts.push(format!("Annotations: {status}"));
    }

    if let Some(status) = &app.export_status {
        parts.push(status.clone());
    }

    if app.debug_bar {
        // Choose a debug panel: if we have grid, pick the top-left grid panel; otherwise pick the first panel
        let debug_panel: Option<&PanelState> = if app.panels.iter().any(|p| p.grid.is_some()) {
            app.panels
                .iter()
                .filter(|p| p.grid.is_some())
                .min_by_key(|p| {
                    let g = p.grid.unwrap();
                    (g.y, g.x)
                })
        } else {
            app.panels.first()
        };

        if let Some(p) = debug_panel {
            let url = p.last_url.as_deref().unwrap_or("-");
            parts.push(format!(
                "last panel: {} | samples={} | url={}",
                p.title, p.last_samples, url
            ));
        }
    }

    parts.join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{GridUnit, PanelType, SeriesView, YAxisMode},
        export::ExportOptions,
        prom::PromClient,
        theme::Theme,
    };
    use ratatui::{Terminal, backend::TestBackend};

    fn test_app() -> AppState {
        AppState::new(
            PromClient::new("http://localhost:9090".to_string()),
            std::time::Duration::from_secs(300),
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(1),
            "Test".to_string(),
            vec![],
            0,
            Theme::default(),
            "dashed-line".to_string(),
            ExportOptions::default(),
        )
    }

    fn graph_panel(title: &str) -> PanelState {
        PanelState {
            title: title.to_string(),
            exprs: vec![],
            legends: vec![],
            query_modes: vec![],
            series: vec![],
            last_error: None,
            last_url: None,
            last_samples: 0,
            grid: None,
            y_axis_mode: crate::app::YAxisMode::Auto,
            panel_type: crate::app::PanelType::Graph,
            thresholds: None,
            min: None,
            max: None,
            autogrid: None,
            display: crate::ui::DisplayFormat::default(),
            options: crate::app::PanelOptions::None,
        }
    }

    fn v2_compatibility_app() -> AppState {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join("dashboards")
            .join("grafana_v2_compatibility.json");
        let dashboard = crate::grafana::load_grafana_dashboard(&path).unwrap();
        let skipped_panels = dashboard.skipped_panels;
        let title = dashboard.title;
        let panels = dashboard
            .queries
            .into_iter()
            .map(|panel| {
                let series = match panel.panel_type {
                    PanelType::Graph => vec![SeriesView {
                        name: "200".to_string(),
                        value: Some(4.0),
                        points: vec![
                            (1_699_999_940.0, 1.0),
                            (1_699_999_970.0, 3.0),
                            (1_700_000_000.0, 4.0),
                        ],
                        visible: true,
                    }],
                    PanelType::Stat => vec![SeriesView {
                        name: "Memory".to_string(),
                        value: Some(128.0),
                        points: vec![
                            (1_699_999_940.0, 120.0),
                            (1_699_999_970.0, 124.0),
                            (1_700_000_000.0, 128.0),
                        ],
                        visible: true,
                    }],
                    other => panic!("unexpected V2 example panel type: {other:?}"),
                };
                PanelState {
                    title: panel.title,
                    exprs: panel.exprs,
                    legends: panel.legends,
                    query_modes: panel.query_modes,
                    series,
                    last_error: None,
                    last_url: None,
                    last_samples: 3,
                    grid: panel.grid.map(|grid| GridUnit {
                        x: grid.x,
                        y: grid.y,
                        w: grid.w,
                        h: grid.h,
                    }),
                    y_axis_mode: YAxisMode::Auto,
                    panel_type: panel.panel_type,
                    thresholds: panel.thresholds,
                    min: panel.min,
                    max: panel.max,
                    autogrid: panel.autogrid,
                    display: panel.display,
                    options: panel.options,
                }
            })
            .collect();
        let mut app = AppState::new(
            PromClient::new("http://127.0.0.1:9".to_string()),
            std::time::Duration::from_secs(60),
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(5),
            format!("{title} (imported)"),
            panels,
            skipped_panels,
            Theme::default(),
            "dashed-line".to_string(),
            ExportOptions::default(),
        );
        app.view_end_ts = 1_700_000_000;
        app
    }

    fn terminal_text(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let area = buffer.area;
        (area.y..area.bottom())
            .map(|y| {
                (area.x..area.right())
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn v2_example_fixed_grid_renders_at_supported_viewports() {
        for (width, height) in [(120, 30), (80, 24)] {
            let mut app = v2_compatibility_app();
            assert_eq!(app.panels.len(), 2);
            assert_eq!(app.selected_panel, 0);
            assert_eq!(app.skipped_panels, 0);
            assert_eq!(app.panels[0].panel_type, PanelType::Graph);
            assert_eq!(app.panels[1].panel_type, PanelType::Stat);
            assert_eq!(
                app.panels[0]
                    .grid
                    .map(|grid| (grid.x, grid.y, grid.w, grid.h)),
                Some((0, 0, 16, 8))
            );
            assert_eq!(
                app.panels[1]
                    .grid
                    .map(|grid| (grid.x, grid.y, grid.w, grid.h)),
                Some((16, 0, 8, 8))
            );

            let rects = crate::ui::visible_panel_rects(Rect::new(0, 0, width, height), &app);
            assert_eq!(rects.len(), 2);
            assert_eq!((rects[0].1, rects[1].1), (0, 1));
            let (left, right) = (rects[0].0, rects[1].0);
            assert_eq!(left.y, right.y);
            assert_eq!(left.height, right.height);
            assert_eq!(left.width, right.width * 2);
            assert!(left.right() <= right.x);

            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();
            let text = terminal_text(&terminal);
            assert!(text.contains("HTTP Request Rate by Status Code"));
            assert!(text.contains("Process Resident"));
            assert!(text.contains("panels=2 (skipped 0)"));
            assert!(!text.contains("No panels"));
            assert!(!text.contains("panic"));
            assert_eq!(
                terminal
                    .backend()
                    .buffer()
                    .cell((left.x, left.y))
                    .unwrap()
                    .fg,
                app.theme.border_selected
            );
            assert_eq!(
                terminal
                    .backend()
                    .buffer()
                    .cell((right.x, right.y))
                    .unwrap()
                    .fg,
                app.theme.border
            );
        }
    }

    #[test]
    fn draw_caches_only_selected_panels_active_filtered_cluster() {
        let mut cpu_deploy = crate::annotations::test_event_at(50.0, "cpu deploy");
        cpu_deploy.tags = vec!["deploy".to_string()];
        cpu_deploy.target = crate::annotations::AnnotationTarget::PanelTitles(
            ["CPU".to_string()].into_iter().collect(),
        );
        let mut cpu_incident = crate::annotations::test_event_at(50.0, "cpu incident");
        cpu_incident.tags = vec!["incident".to_string()];
        cpu_incident.target = crate::annotations::AnnotationTarget::PanelTitles(
            ["CPU".to_string()].into_iter().collect(),
        );
        let mut memory_incident = crate::annotations::test_event_at(50.0, "memory incident");
        memory_incident.tags = vec!["incident".to_string()];
        memory_incident.target = crate::annotations::AnnotationTarget::PanelTitles(
            ["Memory".to_string()].into_iter().collect(),
        );
        let mut app = test_app();
        app.panels = vec![graph_panel("CPU"), graph_panel("Memory")];
        app.view_end_ts = 100;
        app.range = std::time::Duration::from_secs(100);
        app.mode = AppMode::Inspect;
        app.cursor_x = Some(50.0);
        app.annotations = crate::annotations::AnnotationState::from_events_for_test(vec![
            cpu_deploy,
            cpu_incident,
            memory_incident,
        ]);
        app.annotations
            .set_filter(crate::annotations::TagFilter::from_selected([
                "deploy".to_string()
            ]));
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();

        terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();
        assert_eq!(
            app.rendered_annotation_cluster
                .as_ref()
                .unwrap()
                .iter()
                .map(|event| event.text.as_str())
                .collect::<Vec<_>>(),
            vec!["cpu deploy"]
        );

        app.selected_panel = 1;
        terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();
        assert!(app.rendered_annotation_cluster.is_none());

        let mut memory_deploy = crate::annotations::test_event_at(50.0, "memory deploy");
        memory_deploy.tags = vec!["deploy".to_string()];
        memory_deploy.target = crate::annotations::AnnotationTarget::PanelTitles(
            ["Memory".to_string()].into_iter().collect(),
        );
        app.annotations =
            crate::annotations::AnnotationState::from_events_for_test(vec![memory_deploy]);
        app.panels[1].panel_type = crate::app::PanelType::Unknown;
        terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();
        assert!(app.rendered_annotation_cluster.is_none());
    }

    #[test]
    fn footer_composes_annotation_warning_with_export_and_inspect_status() {
        let mut app = test_app();
        app.export_status = Some("Exported frame.svg".to_string());
        app.mode = AppMode::Inspect;
        app.cursor_x = Some(1_700_000_000.0);
        app.annotations =
            crate::annotations::AnnotationState::warning_for_test("events.jsonl:2: invalid time");

        let detail = build_footer_detail(&app);

        assert!(detail.contains("Cursor:"));
        assert!(detail.contains("Annotations: events.jsonl:2: invalid time"));
        assert!(detail.contains("Exported frame.svg"));
    }

    #[test]
    fn footer_composes_annotation_warning_with_sorted_filter_summary() {
        let mut event = crate::annotations::test_event_at(10.0, "deploy");
        event.target = crate::annotations::AnnotationTarget::PanelTitles(
            ["CPU".to_string()].into_iter().collect(),
        );
        let mut app = test_app();
        app.annotations = crate::annotations::AnnotationState::from_events_for_test(vec![event]);
        app.annotations
            .reconcile_targets(&["CPU".to_string(), "CPU".to_string()]);
        app.annotations
            .set_filter(crate::annotations::TagFilter::from_selected([
                "incident".to_string(),
                "deploy".to_string(),
            ]));

        assert_eq!(
            build_footer_detail(&app),
            "Annotations: target \"CPU\" matches 2 graph/timeseries panels; applied to all | tags deploy|incident"
        );
    }

    #[test]
    fn footer_omits_annotation_status_when_disabled() {
        let app = test_app();
        assert!(!build_footer_detail(&app).contains("Annotations:"));
    }
}
