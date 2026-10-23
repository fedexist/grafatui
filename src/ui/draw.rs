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

use super::layout::{DashboardRectKind, centered_rect, visible_dashboard_rects};
use super::panels::render_panel;
use crate::app::{AppMode, AppState, DashboardWorkspace, PanelState};
use crate::{dashboard::DashboardRow, theme::Theme};
use humantime::format_duration;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

#[cfg(test)]
pub(crate) fn draw_ui(frame: &mut Frame, app: &mut AppState) {
    draw_ui_with_tabs(frame, app, None);
}

pub(crate) fn draw_workspace_ui(frame: &mut Frame, workspace: &mut DashboardWorkspace) {
    if workspace.dashboards().len() == 1 {
        draw_ui_with_tabs(frame, workspace.active_mut(), None);
        return;
    }
    let active = workspace.active_index();
    let titles = workspace
        .dashboards()
        .iter()
        .map(|dashboard| dashboard.title.clone())
        .collect::<Vec<_>>();
    draw_ui_with_tabs(frame, workspace.active_mut(), Some((&titles, active)));
}

fn draw_ui_with_tabs(frame: &mut Frame, app: &mut AppState, tabs: Option<(&[String], usize)>) {
    let frame_area = frame.area();
    if let Some((titles, active)) = tabs {
        let tab_area = Rect::new(frame_area.x, frame_area.y, frame_area.width, 1);
        let geometry = super::tab_bar_geometry(tab_area, titles, Some(active), 0);
        super::render_tab_bar(frame, &geometry, &app.theme, true);
    }
    let size = super::dashboard_viewport(frame_area, tabs.is_some());

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
    let title_text = if tabs.is_some() {
        format!("{} — {}", app.title, format_duration(app.range))
    } else {
        format!(
            "{} — range={} step={}  panels={}  {}(r refresh, +/- range, [] pan, 0 live, q quit)",
            app.title,
            format_duration(app.range),
            format_duration(app.default_intervals().step),
            normal_panel_count(app),
            if app.is_live() { "" } else { "⏸ PAUSED " }
        )
    };
    let title_block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(title_text).alignment(Alignment::Center));
    frame.render_widget(title_block, chunks[0]);

    // Charts area: use Grafana grid if any panel has it, else fallback to 2-column flow
    let area = chunks[1];
    let charts_block = Block::default().borders(Borders::ALL);
    frame.render_widget(charts_block, area);
    let mut selected_rendered_cluster = None;
    for item in visible_dashboard_rects(size, app) {
        match item.kind {
            DashboardRectKind::Panel { index, .. } => {
                if let Some(panel) = app.panels.get(index) {
                    let is_selected = app.selected_item == Some(item.id);
                    let rendered_cluster = render_panel(
                        frame,
                        item.rect,
                        super::panel_render_context(app, &item).unwrap(),
                        panel,
                        app,
                        is_selected,
                        app.cursor_x,
                    );
                    if is_selected {
                        selected_rendered_cluster = rendered_cluster;
                    }
                }
            }
            DashboardRectKind::Row { row_id, depth, .. } => {
                if let Some(row) = app.layout.row(row_id) {
                    render_row_header(
                        frame,
                        item.rect,
                        row,
                        depth,
                        app.selected_item == Some(item.id),
                        &app.theme,
                    );
                }
            }
            DashboardRectKind::Tabs { group_id, depth } => {
                if let Some(group) = app.layout.tabs(group_id) {
                    let titles = group
                        .tabs
                        .iter()
                        .map(|tab| tab.title.clone())
                        .collect::<Vec<_>>();
                    let geometry = super::tab_bar_geometry(item.rect, &titles, group.active, depth);
                    super::render_tab_bar(
                        frame,
                        &geometry,
                        &app.theme,
                        app.selected_item == Some(item.id),
                    );
                }
            }
            DashboardRectKind::TabEmpty { .. } => frame.render_widget(
                Line::styled(
                    "  No supported panels in this tab",
                    Style::default().fg(app.theme.text),
                ),
                item.rect,
            ),
        }
    }
    app.rendered_annotation_cluster = selected_rendered_cluster;

    // Footer / Status bar
    let errors = app.panels.iter().filter(|p| p.last_error.is_some()).count();
    let panel_count_display =
        if app.mode == AppMode::Fullscreen || app.mode == AppMode::FullscreenInspect {
            "1 (Fullscreen)".to_string()
        } else {
            normal_panel_count(app)
        };

    let mode_display = match app.mode {
        AppMode::Normal => "NORMAL",
        AppMode::Search => "SEARCH",
        AppMode::Fullscreen => "FULLSCREEN",
        AppMode::Inspect => "INSPECT",
        AppMode::FullscreenInspect => "FULLSCREEN INSPECT",
    };

    let navigation_hint = if tabs.is_some() {
        "Tab/Shift+Tab dashboards, ↑/↓ navigate"
    } else if app.selected_tab_group_id().is_some() {
        "←/→ switch tab, Enter enter, ↑/↓ navigate"
    } else {
        "↑/↓ navigate"
    };
    let body_hint = if app.mode != AppMode::Search
        && app.annotation_modal.is_none()
        && super::panel_body_viewports(size, app).iter().any(|b| {
            Some(b.index) == app.selected_panel_index()
                && b.metrics.capacity > 0
                && b.metrics.max_local_offset() > 0
        }) {
        "Ctrl+↑/↓ PgUp/PgDn Home/End body | "
    } else {
        ""
    };
    let summary = if tabs.is_some() {
        format!(
            "Mode: {}{} | range={} step={} | panels={} | keys: Tab/Shift+Tab, ↑/↓, r, +/-, q",
            mode_display,
            if app.recording.is_some() { " REC" } else { "" },
            format_duration(app.range),
            format_duration(app.default_intervals().step),
            panel_count_display,
        )
    } else {
        format!(
            "{body_hint}Mode: {}{} | Prom: {} | range={} step={:?} refresh={} | grid={} | panels={} (skipped {}) errors={} | keys: {navigation_hint}, r refresh, e export, Ctrl+E record, +/- range, q quit, ? debug:{}",
            mode_display,
            if app.recording.is_some() { " REC" } else { "" },
            app.prometheus.base,
            format_duration(app.range),
            app.default_intervals().step,
            format_duration(app.refresh_every),
            if app.autogrid_enabled { "on" } else { "off" },
            panel_count_display,
            app.skipped_panels,
            errors,
            if app.debug_bar { "on" } else { "off" }
        )
    };

    let detail = build_footer_detail(app);

    // Statuses such as export results come first: the summary alone usually
    // fills the two footer lines, which would hide them.
    let text = if detail.is_empty() {
        Text::from(summary)
    } else {
        let detail = if let Some(warning) = app.variable_warning()
            && let Some(rest) = detail.strip_prefix(&warning)
        {
            Line::from(vec![
                Span::styled(
                    warning,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(rest.to_string()),
            ])
        } else {
            Line::raw(detail)
        };
        Text::from(vec![detail, Line::raw(summary)])
    };
    let footer = Paragraph::new(text).wrap(Wrap { trim: true });
    frame.render_widget(footer, chunks[2]);

    // Search Popup
    if app.mode == AppMode::Search {
        let area = centered_rect(60, 20, size);
        let block = Block::default()
            .title(" Search Dashboard ")
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
            .filter_map(|item| match item {
                crate::dashboard::DashboardItemId::Row(row_id) => {
                    let row = app.layout.row(*row_id)?;
                    let marker = if row.collapsed { '▶' } else { '▼' };
                    Some(ListItem::new(format!("{marker} {}", row.title)))
                }
                crate::dashboard::DashboardItemId::Panel(index) => {
                    let panel = app.panels.get(*index)?;
                    Some(ListItem::new(format!("• {}", panel.title)))
                }
                crate::dashboard::DashboardItemId::Tabs(group_id) => {
                    let group = app.layout.tabs(*group_id)?;
                    let title = group
                        .active
                        .and_then(|index| {
                            group
                                .tabs
                                .get(index)
                                .map(|tab| super::tab_title(&tab.title, index))
                        })
                        .unwrap_or_else(|| "No tabs".to_string());
                    Some(ListItem::new(format!("▰ {title}")))
                }
            })
            .collect();
        let list = List::new(results)
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

fn normal_panel_count(app: &AppState) -> String {
    let total = app.panels.len();
    let visible = if app
        .layout
        .items
        .iter()
        .all(|item| matches!(item, crate::dashboard::DashboardLayoutItem::Panel(_)))
    {
        total
    } else {
        app.layout.visible_panel_count()
    };
    if visible == total {
        total.to_string()
    } else {
        format!("{visible}/{total}")
    }
}

pub(crate) fn render_row_header(
    frame: &mut Frame,
    rect: Rect,
    row: &DashboardRow,
    depth: usize,
    selected: bool,
    theme: &Theme,
) {
    let marker = if row.collapsed { '▶' } else { '▼' };
    let text = format!("{marker} {}{}", "  ".repeat(depth), row.title);
    let style = Style::default()
        .fg(if selected {
            theme.border_selected
        } else {
            theme.border
        })
        .add_modifier(if selected {
            Modifier::BOLD
        } else {
            Modifier::empty()
        });
    frame.render_widget(Paragraph::new(text).style(style), rect);
}

fn build_footer_detail(app: &AppState) -> String {
    let mut parts = Vec::new();

    if let Some(warning) = app.variable_warning() {
        parts.push(warning);
    }

    if let Some(notice) = selected_panel_notice(app) {
        parts.push(notice);
    }

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

fn selected_panel_notice(app: &AppState) -> Option<String> {
    let panel = app.panels.get(app.selected_panel_index()?)?;
    let mut messages = Vec::new();
    if !panel.series.is_empty()
        && let Some(error) = &panel.last_error
    {
        messages.extend(error.lines().map(ToString::to_string));
    }
    messages.extend(
        panel
            .notices
            .warnings
            .iter()
            .map(|warning| format!("warning: {warning}")),
    );
    let first = messages.first()?;
    let more = match messages.len() {
        0 | 1 => String::new(),
        count => format!(" (+{} more)", count - 1),
    };
    Some(format!("{}: {first}{more}", panel.title))
}

#[cfg(test)]
pub(super) mod tests {
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

    #[test]
    fn workspace_title_shows_all_top_level_tabs_and_active_dashboard() {
        use crate::dashboard::DashboardLayout;

        let mut alpha = test_app();
        alpha.title = "Alpha".into();
        alpha.panels = vec![graph_panel("Alpha CPU")];
        alpha.apply_layout(DashboardLayout::flat(1));
        let mut beta = test_app();
        beta.title = "Beta".into();
        beta.panels = vec![graph_panel("Beta Memory")];
        beta.apply_layout(DashboardLayout::flat(1));
        alpha.view_end_ts = 1_783_080_000;
        beta.view_end_ts = 1_783_080_000;
        let mut workspace = DashboardWorkspace::new(vec![alpha, beta]);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();

        terminal
            .draw(|frame| draw_workspace_ui(frame, &mut workspace))
            .unwrap();
        let initial = terminal_text(&terminal);
        assert!(initial.contains("* Alpha"));
        assert!(initial.contains("Beta"));
        assert!(initial.contains("Alpha CPU"));
        assert!(!initial.contains("Beta Memory"));
        if let Ok(directory) = std::env::var("GRAFATUI_MULTI_DASHBOARD_CAPTURE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            capture_buffer(&terminal, &directory.join("alpha-active-100x30.json"));
        }

        workspace.switch_next();
        terminal
            .draw(|frame| draw_workspace_ui(frame, &mut workspace))
            .unwrap();
        let switched = terminal_text(&terminal);
        assert!(switched.contains("Alpha"));
        assert!(switched.contains("* Beta"));
        assert!(switched.contains("Beta Memory"));
        assert!(!switched.contains("Alpha CPU"));
        if let Ok(directory) = std::env::var("GRAFATUI_MULTI_DASHBOARD_CAPTURE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            capture_buffer(&terminal, &directory.join("beta-active-100x30.json"));
        }
    }

    #[test]
    fn single_dashboard_workspace_keeps_the_legacy_screen() {
        let mut legacy_app = test_app();
        legacy_app.title = "Solo".into();
        let mut workspace_app = test_app();
        workspace_app.title = "Solo".into();
        let mut workspace_state = DashboardWorkspace::new(vec![workspace_app]);
        let mut legacy = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut workspace = Terminal::new(TestBackend::new(80, 24)).unwrap();

        legacy
            .draw(|frame| draw_ui(frame, &mut legacy_app))
            .unwrap();
        workspace
            .draw(|frame| draw_workspace_ui(frame, &mut workspace_state))
            .unwrap();

        assert_eq!(terminal_text(&workspace), terminal_text(&legacy));
    }

    #[test]
    fn workspace_tabs_keep_the_active_dashboard_visible_when_narrow() {
        use crate::dashboard::DashboardLayout;

        let dashboards = ["Alpha", "Beta", "Gamma"]
            .into_iter()
            .map(|name| {
                let mut dashboard = test_app();
                dashboard.title = name.to_string();
                dashboard.panels = vec![graph_panel(&format!("{name} panel"))];
                dashboard.apply_layout(DashboardLayout::flat(1));
                dashboard.view_end_ts = 1_783_080_000;
                dashboard
            })
            .collect();
        let mut workspace = DashboardWorkspace::new(dashboards);
        workspace.switch_next();
        workspace.switch_next();
        let mut terminal = Terminal::new(TestBackend::new(24, 16)).unwrap();

        terminal
            .draw(|frame| draw_workspace_ui(frame, &mut workspace))
            .unwrap();
        let text = terminal_text(&terminal);
        assert!(text.contains("* Gamma"), "{text}");
        assert!(text.contains("Gamma — 5m"), "{text}");

        let titles = workspace
            .dashboards()
            .iter()
            .map(|dashboard| dashboard.title.clone())
            .collect::<Vec<_>>();
        let geometry = super::super::tab_bar_geometry(
            ratatui::layout::Rect::new(0, 0, 24, 1),
            &titles,
            Some(workspace.active_index()),
            0,
        );
        assert!(
            geometry
                .segments
                .iter()
                .all(|segment| segment.rect.right() <= 24)
        );
        if let Ok(directory) = std::env::var("GRAFATUI_MULTI_DASHBOARD_CAPTURE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            capture_buffer(&terminal, &directory.join("gamma-active-24x16.json"));
        }
    }

    fn graph_panel(title: &str) -> PanelState {
        PanelState {
            title: title.to_string(),
            exprs: vec![],
            legends: vec![],
            query_modes: vec![],
            series: vec![],
            last_error: None,
            notices: Default::default(),
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
            display_name_template: None,
            options: crate::app::PanelOptions::None,
            resolution: Default::default(),
        }
    }

    fn graph_panel_with_data(title: &str) -> PanelState {
        let mut panel = graph_panel(title);
        panel.exprs = vec!["up".to_string()];
        panel.series = vec![SeriesView {
            query_index: 0,
            name: "api".to_string(),
            display_name: None,
            value: Some(2.0),
            points: vec![(1_700_000_000.0, 1.0), (1_700_000_060.0, 2.0)],
            visible: true,
        }];
        panel.last_samples = 2;
        panel
    }

    #[test]
    fn stale_panel_keeps_chart_and_explains_failure() {
        let mut app = test_app();
        let mut panel = graph_panel_with_data("Requests");
        panel.last_error = Some("query_range failed for `up`: bad expression".to_string());
        panel.notices.stale_queries = 1;
        app.panels = vec![panel];
        app.apply_layout(crate::dashboard::DashboardLayout::flat(1));
        app.view_end_ts = 1_700_000_060;
        assert_eq!(app.panels[0].notices.stale_queries, 1);
        assert!(app.panels[0].last_error.is_some());
        assert_eq!(app.panels[0].series.len(), 1);

        let text = last_good_contract_capture(&mut app, 120, 40, "stale-total");

        assert!(text.contains("Requests ⚠ stale: queries failed"), "{text}");
        assert!(text.contains("api"), "{text}");
        assert!(!text.contains("ERROR"), "{text}");
        assert!(
            text.contains("Requests: query_range failed for `up`: bad expression"),
            "{text}"
        );
    }

    #[test]
    fn warning_panel_keeps_chart_and_explains_warning() {
        let mut app = test_app();
        let mut panel = graph_panel_with_data("Requests");
        panel.notices.warnings = vec![
            "partial response".to_string(),
            "metric might not be a counter".to_string(),
        ];
        app.panels = vec![panel];
        app.apply_layout(crate::dashboard::DashboardLayout::flat(1));
        app.view_end_ts = 1_700_000_060;
        assert_eq!(app.panels[0].notices.warnings.len(), 2);
        assert!(app.panels[0].last_error.is_none());

        let text = last_good_contract_capture(&mut app, 100, 30, "warning");

        assert!(text.contains("Requests ⚠ warning"), "{text}");
        assert!(text.contains("api"), "{text}");
        assert!(
            text.contains("Requests: warning: partial response (+1 more)"),
            "{text}"
        );

        app.panels[0].notices = Default::default();
        let recovered = last_good_contract_capture(&mut app, 100, 30, "warning-cleared");
        assert!(!recovered.contains("⚠ warning"), "{recovered}");
        assert!(!recovered.contains("partial response"), "{recovered}");
    }

    #[test]
    fn variable_failure_keeps_dashboard_data_and_warns_in_footer() {
        let mut app = test_app();
        app.vars.insert("instance".into(), "node-a".into());
        app.variable_errors
            .insert("instance".into(), "request timed out".into());
        app.panels = vec![graph_panel_with_data("Requests")];
        app.apply_layout(crate::dashboard::DashboardLayout::flat(1));
        app.view_end_ts = 1_700_000_060;
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

        terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();

        let text = terminal_text(&terminal);
        assert_eq!(app.vars.get("instance").map(String::as_str), Some("node-a"));
        assert_eq!(app.panels[0].series.len(), 1);
        assert!(text.contains("api"), "{text}");
        assert!(
            text.contains("Variable instance failed: request timed out"),
            "{text}"
        );
        assert!(!text.contains("old cluster failure"), "{text}");
        let warning_cell = terminal.backend().buffer().cell((0, 22)).unwrap();
        assert_eq!(warning_cell.symbol(), "V");
        assert_eq!(warning_cell.fg, Color::Yellow);
        assert!(warning_cell.modifier.contains(Modifier::BOLD));

        if let Ok(root) = std::env::var("GRAFATUI_VARIABLE_WARNING_CAPTURE_DIR") {
            let directory = std::path::PathBuf::from(root);
            std::fs::create_dir_all(&directory).unwrap();
            capture_buffer(&terminal, &directory.join("footer-warning-80x24.json"));
        }
    }

    #[test]
    fn partial_panel_marks_only_the_failed_query_as_stale() {
        let mut app = test_app();
        let mut panel = graph_panel_with_data("Requests");
        panel.exprs.push("errors".to_string());
        panel.series.push(SeriesView {
            query_index: 1,
            name: "errors".to_string(),
            display_name: None,
            value: Some(1.0),
            points: vec![(1_700_000_000.0, 1.0), (1_700_000_060.0, 1.0)],
            visible: true,
        });
        panel.last_error = Some("query_range failed for `errors`: bad expression".to_string());
        panel.notices.stale_queries = 1;
        app.panels = vec![panel];
        app.apply_layout(crate::dashboard::DashboardLayout::flat(1));
        app.view_end_ts = 1_700_000_060;
        assert_eq!(app.panels[0].notices.stale_queries, 1);
        assert_eq!(app.panels[0].series.len(), 2);

        let text = last_good_contract_capture(&mut app, 120, 40, "stale-partial");

        assert!(text.contains("Requests ⚠ partial: stale query"), "{text}");
        assert!(text.contains("api"), "{text}");
        assert!(text.contains("errors"), "{text}");
        assert!(!text.contains("ERROR"), "{text}");
    }

    fn nested_row_app() -> AppState {
        use crate::dashboard::{
            DashboardItemId, DashboardLayout, DashboardLayoutItem, DashboardRow, RowId,
        };

        let mut app = test_app();
        app.panels = vec![graph_panel("Visible child"), graph_panel("Collapsed child")];
        app.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::Row(
            DashboardRow::new(
                RowId::new(0),
                "Expanded",
                false,
                false,
                vec![
                    DashboardLayoutItem::Panel(0),
                    DashboardLayoutItem::Row(DashboardRow::new(
                        RowId::new(1),
                        "Nested",
                        true,
                        false,
                        vec![DashboardLayoutItem::Panel(1)],
                    )),
                ],
            ),
        )]));
        app.selected_item = Some(DashboardItemId::Row(RowId::new(0)));
        app
    }

    #[test]
    fn tabs_render_active_marker_focus_and_only_active_panel() {
        use crate::dashboard::{
            DashboardItemId, DashboardLayout, DashboardLayoutItem, DashboardTab, DashboardTabs,
            TabGroupId,
        };

        let id = TabGroupId::new(0);
        let mut app = test_app();
        app.view_end_ts = 1_783_080_000;
        app.panels = vec![graph_panel("CPU panel"), graph_panel("Memory panel")];
        app.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::Tabs(
            DashboardTabs::new(
                id,
                vec![
                    DashboardTab {
                        title: "CPU".into(),
                        children: vec![DashboardLayoutItem::Panel(0)],
                    },
                    DashboardTab {
                        title: "Memory".into(),
                        children: vec![DashboardLayoutItem::Panel(1)],
                    },
                ],
            ),
        )]));
        app.selected_item = Some(DashboardItemId::Tabs(id));
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();

        terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();
        let text = terminal_text(&terminal);

        assert!(text.contains("* CPU Memory"));
        assert!(text.contains("CPU panel"));
        assert!(!text.contains("Memory panel"));
        let bar = visible_dashboard_rects(Rect::new(0, 0, 100, 40), &app)
            .into_iter()
            .find(|item| item.id == DashboardItemId::Tabs(id))
            .unwrap();
        let active_cell = terminal
            .backend()
            .buffer()
            .cell((bar.rect.x, bar.rect.y))
            .unwrap();
        assert!(active_cell.modifier.contains(Modifier::BOLD));
        assert!(active_cell.modifier.contains(Modifier::UNDERLINED));

        if let Ok(directory) = std::env::var("GRAFATUI_TABS_CAPTURE_DIR") {
            let directory = std::path::PathBuf::from(directory).join("tabs-switch");
            std::fs::create_dir_all(&directory).unwrap();
            capture_buffer(&terminal, &directory.join("initial-100x40.json"));

            app.layout.set_active_tab(id, 1).unwrap();
            app.selected_item = Some(DashboardItemId::Tabs(id));
            terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();
            let switched = terminal_text(&terminal);
            assert!(switched.contains("CPU * Memory"));
            assert!(switched.contains("Memory panel"));
            assert!(!switched.contains("CPU panel"));
            capture_buffer(&terminal, &directory.join("switched-100x40.json"));

            app.enter_selected_tab();
            terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();
            assert_eq!(app.selected_item, Some(DashboardItemId::Panel(1)));
            capture_buffer(&terminal, &directory.join("entered-100x40.json"));
        }
    }

    #[test]
    fn tabs_render_empty_overflow_nested_and_scrolled_contracts() {
        use crate::dashboard::{
            DashboardItemId, DashboardLayout, DashboardLayoutItem, DashboardRow, DashboardTab,
            DashboardTabs, RowId, TabGroupId,
        };

        let mut empty = test_app();
        empty.view_end_ts = 1_783_080_000;
        empty.apply_layout(DashboardLayout::new(vec![
            DashboardLayoutItem::Tabs(DashboardTabs::new(TabGroupId::new(0), vec![])),
            DashboardLayoutItem::Tabs(DashboardTabs::new(
                TabGroupId::new(1),
                vec![DashboardTab {
                    title: "Empty".into(),
                    children: vec![],
                }],
            )),
        ]));
        let empty_text = draw_contract_capture(&mut empty, 40, 12, "tabs-empty");
        assert!(empty_text.contains("No tabs"));
        assert!(empty_text.contains("No supported panels in this tab"));

        let overflow_id = TabGroupId::new(2);
        let mut overflow = test_app();
        overflow.view_end_ts = 1_783_080_000;
        overflow.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::Tabs(
            DashboardTabs::new(
                overflow_id,
                ["Overview", "服务", "e\u{301}rrors", "Latency", "Final"]
                    .into_iter()
                    .map(|title| DashboardTab {
                        title: title.into(),
                        children: vec![],
                    })
                    .collect(),
            ),
        )]));
        overflow.layout.set_active_tab(overflow_id, 4).unwrap();
        overflow.selected_item = Some(DashboardItemId::Tabs(overflow_id));
        let overflow_text = draw_contract_capture(&mut overflow, 24, 12, "tabs-overflow");
        assert!(overflow_text.contains("<"));
        assert!(overflow_text.contains("* Final"));

        let outer_id = TabGroupId::new(3);
        let inner_id = TabGroupId::new(4);
        let mut nested = test_app();
        nested.view_end_ts = 1_783_080_000;
        nested.panels = vec![graph_panel("Inactive")];
        nested.apply_layout(DashboardLayout::new(vec![DashboardLayoutItem::Tabs(
            DashboardTabs::new(
                outer_id,
                vec![
                    DashboardTab {
                        title: "Nested".into(),
                        children: vec![DashboardLayoutItem::Tabs(DashboardTabs::new(
                            inner_id,
                            vec![
                                DashboardTab {
                                    title: "First".into(),
                                    children: vec![],
                                },
                                DashboardTab {
                                    title: "Second".into(),
                                    children: vec![DashboardLayoutItem::Row(DashboardRow::new(
                                        RowId::new(0),
                                        "Collapsed",
                                        true,
                                        false,
                                        vec![],
                                    ))],
                                },
                            ],
                        ))],
                    },
                    DashboardTab {
                        title: "Other".into(),
                        children: vec![DashboardLayoutItem::Panel(0)],
                    },
                ],
            ),
        )]));
        nested.layout.set_active_tab(inner_id, 1).unwrap();
        nested.selected_item = Some(DashboardItemId::Tabs(outer_id));
        let nested_text = draw_contract_capture(&mut nested, 100, 40, "tabs-nested");
        assert!(nested_text.contains("* Nested Other"));
        assert!(nested_text.contains("First * Second"));
        assert!(nested_text.contains('▶'));
        assert!(nested_text.contains("Collapsed"));
        assert!(!nested_text.contains("Inactive"));

        let scroll_id = TabGroupId::new(5);
        let mut scrolled = test_app();
        scrolled.view_end_ts = 1_783_080_000;
        scrolled.panels = vec![graph_panel("Above one"), graph_panel("Above two")];
        scrolled.apply_layout(DashboardLayout::new(vec![
            DashboardLayoutItem::Panel(0),
            DashboardLayoutItem::Panel(1),
            DashboardLayoutItem::Tabs(DashboardTabs::new(
                scroll_id,
                vec![DashboardTab {
                    title: "Below".into(),
                    children: vec![],
                }],
            )),
        ]));
        scrolled.selected_item = Some(DashboardItemId::Tabs(scroll_id));
        super::super::scroll_selected_into_view(Rect::new(0, 0, 100, 12), &mut scrolled);
        assert!(scrolled.vertical_scroll > 0);
        let scroll_text = draw_contract_capture(&mut scrolled, 100, 12, "tabs-scroll");
        assert!(scroll_text.contains("* Below"));
    }

    fn draw_contract_capture(app: &mut AppState, width: u16, height: u16, name: &str) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw_ui(frame, app)).unwrap();
        if let Ok(root) = std::env::var("GRAFATUI_TABS_CAPTURE_DIR") {
            let directory = std::path::PathBuf::from(root).join(name);
            std::fs::create_dir_all(&directory).unwrap();
            capture_buffer(
                &terminal,
                &directory.join(format!("{name}-{width}x{height}.json")),
            );
        }
        terminal_text(&terminal)
    }

    fn last_good_contract_capture(
        app: &mut AppState,
        width: u16,
        height: u16,
        name: &str,
    ) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw_ui(frame, app)).unwrap();
        if let Ok(root) = std::env::var("GRAFATUI_LAST_GOOD_CAPTURE_DIR") {
            let directory = std::path::PathBuf::from(root).join(name);
            std::fs::create_dir_all(&directory).unwrap();
            capture_buffer(
                &terminal,
                &directory.join(format!("{name}-{width}x{height}.json")),
            );
        }
        terminal_text(&terminal)
    }

    pub(crate) fn capture_buffer(terminal: &Terminal<TestBackend>, path: &std::path::Path) {
        let buffer = terminal.backend().buffer();
        let cells = (0..buffer.area.height)
            .flat_map(|y| {
                (0..buffer.area.width).map(move |x| {
                    let cell = buffer.cell((x, y)).unwrap();
                    let modifiers = [
                        (Modifier::BOLD, "bold"),
                        (Modifier::DIM, "dim"),
                        (Modifier::ITALIC, "italic"),
                        (Modifier::UNDERLINED, "underlined"),
                        (Modifier::REVERSED, "reversed"),
                        (Modifier::CROSSED_OUT, "crossed_out"),
                    ]
                    .into_iter()
                    .filter_map(|(flag, name)| cell.modifier.contains(flag).then_some(name))
                    .collect::<Vec<_>>();
                    serde_json::json!({
                        "x": x,
                        "y": y,
                        "symbol": cell.symbol(),
                        "fg": test_color_hex(cell.fg, "#d0d0d0"),
                        "bg": test_color_hex(cell.bg, "#101010"),
                        "modifiers": modifiers,
                    })
                })
            })
            .collect::<Vec<_>>();
        let capture = serde_json::json!({
            "version": 1,
            "width": buffer.area.width,
            "height": buffer.area.height,
            "cell_width": 8,
            "cell_height": 16,
            "default_fg": "#d0d0d0",
            "default_bg": "#101010",
            "cells": cells,
            "cursor": serde_json::Value::Null,
        });
        std::fs::write(path, serde_json::to_vec_pretty(&capture).unwrap()).unwrap();
    }

    fn test_color_hex(color: Color, reset: &str) -> String {
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
            Color::Indexed(index) if index < 16 => {
                const ANSI: [(u8, u8, u8); 16] = [
                    (0, 0, 0),
                    (128, 0, 0),
                    (0, 128, 0),
                    (128, 128, 0),
                    (0, 0, 128),
                    (128, 0, 128),
                    (0, 128, 128),
                    (192, 192, 192),
                    (128, 128, 128),
                    (255, 0, 0),
                    (0, 255, 0),
                    (255, 255, 0),
                    (0, 0, 255),
                    (255, 0, 255),
                    (0, 255, 255),
                    (255, 255, 255),
                ];
                ANSI[usize::from(index)]
            }
            Color::Indexed(index @ 16..=231) => {
                const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
                let cube = index - 16;
                (
                    LEVELS[usize::from(cube / 36)],
                    LEVELS[usize::from((cube / 6) % 6)],
                    LEVELS[usize::from(cube % 6)],
                )
            }
            Color::Indexed(index) => {
                let gray = 8 + (index - 232) * 10;
                (gray, gray, gray)
            }
        };
        format!("#{red:02x}{green:02x}{blue:02x}")
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
                        query_index: 0,
                        name: "200".to_string(),
                        display_name: None,
                        value: Some(4.0),
                        points: vec![
                            (1_699_999_940.0, 1.0),
                            (1_699_999_970.0, 3.0),
                            (1_700_000_000.0, 4.0),
                        ],
                        visible: true,
                    }],
                    PanelType::Stat => vec![SeriesView {
                        query_index: 0,
                        name: "Memory".to_string(),
                        display_name: None,
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
                    notices: Default::default(),
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
                    display_name_template: None,
                    options: panel.options,
                    resolution: panel.resolution,
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

    fn rect_text(terminal: &Terminal<TestBackend>, rect: Rect) -> String {
        let buffer = terminal.backend().buffer();
        (rect.y..rect.bottom())
            .map(|y| {
                (rect.x..rect.right())
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn rows_render_disclosures_indentation_selection_and_visible_count() {
        let mut app = nested_row_app();
        let row_rects =
            super::super::layout::visible_dashboard_rects(Rect::new(0, 0, 100, 40), &app);
        let expanded = row_rects[0].rect;
        let nested = row_rects[2].rect;
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();

        terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();

        let text = terminal_text(&terminal);
        assert!(text.contains("▼ Expanded"));
        assert!(text.contains("▶   Nested"));
        assert!(text.contains("Visible child"));
        assert!(!text.contains("Collapsed child"));
        assert!(text.contains("panels=1/2"));
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((expanded.x, expanded.y))
                .unwrap()
                .fg,
            app.theme.border_selected
        );
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((nested.x, nested.y))
                .unwrap()
                .fg,
            app.theme.border
        );
    }

    #[test]
    fn row_free_dashboard_uses_legacy_count_without_fraction() {
        let mut app = test_app();
        app.panels = vec![graph_panel("CPU"), graph_panel("Memory")];
        app.apply_layout(crate::dashboard::DashboardLayout::flat(2));
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();

        terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();

        let text = terminal_text(&terminal);
        assert!(text.contains("panels=2"));
        assert!(!text.contains("panels=2/2"));
        assert!(!text.contains('▼'));
        assert!(!text.contains('▶'));
    }

    #[test]
    fn imported_exact_value_mapping_renders_stat_text() {
        let path = std::env::temp_dir().join("grafatui-value-mapping-render.json");
        std::fs::write(
            &path,
            r#"{
                "title": "Mapped",
                "panels": [{
                    "type": "stat",
                    "title": "Service health",
                    "targets": [{"expr": "up"}],
                    "fieldConfig": {"defaults": {"mappings": [{
                        "type": "value",
                        "options": {"1": {"text": "Healthy"}}
                    }]}}
                }]
            }"#,
        )
        .unwrap();
        let imported = crate::grafana::load_grafana_dashboard(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        let mut app = test_app();
        let mut panel = graph_panel("Service health");
        panel.panel_type = PanelType::Stat;
        panel.display = imported.queries[0].display.clone();
        panel.series = vec![SeriesView {
            query_index: 0,
            name: "up".to_string(),
            display_name: None,
            value: Some(1.0),
            points: vec![(1_700_000_000.0, 1.0)],
            visible: true,
        }];
        app.panels = vec![panel];
        app.apply_layout(crate::dashboard::DashboardLayout::flat(1));
        assert_eq!(app.panels[0].series[0].value, Some(1.0));

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();

        let text = terminal_text(&terminal);
        assert!(text.contains("Service health"), "{text}");
        assert!(text.contains("Healthy"), "{text}");
        assert!(!text.contains("1.00"), "{text}");

        if let Ok(root) = std::env::var("GRAFATUI_VALUE_MAPPING_CAPTURE_DIR") {
            let directory = std::path::PathBuf::from(root);
            std::fs::create_dir_all(&directory).unwrap();
            capture_buffer(&terminal, &directory.join("mapped-stat-80x24.json"));
        }
    }

    #[test]
    fn exact_value_mapping_formats_value_panels_and_graph_legend() {
        for panel_type in [
            PanelType::Gauge,
            PanelType::BarGauge,
            PanelType::Table,
            PanelType::Graph,
        ] {
            let mut app = test_app();
            let mut panel = graph_panel("Service health");
            panel.panel_type = panel_type;
            panel.display.value_mappings = vec![crate::ui::ValueMapping {
                value: 1.0,
                text: "Healthy".to_string(),
            }];
            panel.series = vec![SeriesView {
                query_index: 0,
                name: "up".to_string(),
                display_name: None,
                value: Some(1.0),
                points: vec![(1_700_000_000.0, 1.0)],
                visible: true,
            }];
            app.panels = vec![panel];
            app.apply_layout(crate::dashboard::DashboardLayout::flat(1));

            let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
            terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();

            let text = terminal_text(&terminal);
            assert!(text.contains("Healthy"), "{panel_type:?}: {text}");
            assert_eq!(app.panels[0].series[0].value, Some(1.0));
        }
    }

    #[test]
    fn summary_panels_display_reduced_scalar_instead_of_latest_point() {
        for panel_type in [PanelType::Stat, PanelType::Gauge, PanelType::BarGauge] {
            let mut app = test_app();
            let mut panel = graph_panel("Summary CPU");
            panel.panel_type = panel_type;
            panel.min = Some(0.0);
            panel.max = Some(10.0);
            panel.series = vec![SeriesView {
                query_index: 0,
                name: "cpu".to_string(),
                display_name: None,
                value: Some(4.0),
                points: vec![
                    (1_700_000_000.0, 1.0),
                    (1_700_000_060.0, 3.0),
                    (1_700_000_120.0, 8.0),
                ],
                visible: true,
            }];
            app.panels = vec![panel];
            app.apply_layout(crate::dashboard::DashboardLayout::flat(1));
            for (width, height) in [(80, 24), (60, 20)] {
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();
                let text = terminal_text(&terminal);
                assert!(text.contains("Summary CPU"), "{panel_type:?}: {text}");
                assert!(text.contains("4.00"), "{panel_type:?}: {text}");
                assert!(!text.contains("8.00"), "{panel_type:?}: {text}");
                if panel_type == PanelType::Gauge
                    && let Ok(root) = std::env::var("GRAFATUI_REDUCE_CAPTURE_DIR")
                {
                    let directory = std::path::PathBuf::from(root);
                    std::fs::create_dir_all(&directory).unwrap();
                    capture_buffer(
                        &terminal,
                        &directory.join(format!("gauge-{width}x{height}.json")),
                    );
                }
            }
        }
    }

    #[test]
    fn gauge_uses_no_value_for_visible_null_samples() {
        let render = |value: Option<f64>,
                      has_visible_series: bool,
                      no_value: Option<&str>,
                      min: f64,
                      viewport: (u16, u16)| {
            let mut app = test_app();
            let mut panel = graph_panel("Cache health");
            panel.panel_type = PanelType::Gauge;
            panel.min = Some(min);
            panel.max = Some(100.0);
            panel.display.no_value = no_value.map(str::to_owned);
            if has_visible_series {
                panel.series = vec![SeriesView {
                    query_index: 0,
                    name: "cache_hit_ratio".to_string(),
                    display_name: None,
                    value,
                    points: vec![],
                    visible: true,
                }];
            }
            app.panels = vec![panel];
            app.apply_layout(crate::dashboard::DashboardLayout::flat(1));
            if has_visible_series {
                assert_eq!(app.panels[0].series[0].value, value);
            }

            let mut terminal = Terminal::new(TestBackend::new(viewport.0, viewport.1)).unwrap();
            terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();
            (terminal_text(&terminal), terminal)
        };
        let capture = |terminal: &Terminal<TestBackend>, name: &str, viewport: (u16, u16)| {
            if let Ok(root) = std::env::var("GRAFATUI_GAUGE_NO_VALUE_CAPTURE_DIR") {
                let directory = std::path::PathBuf::from(root);
                std::fs::create_dir_all(&directory).unwrap();
                capture_buffer(
                    terminal,
                    &directory.join(format!("{name}-{}x{}.json", viewport.0, viewport.1)),
                );
            }
        };

        let (configured, terminal) = render(None, true, Some("n/a"), 0.0, (80, 24));
        assert!(configured.contains("n/a (cache_hit_ratio)"), "{configured}");
        assert!(
            !configured.contains("0.00 (cache_hit_ratio)"),
            "{configured}"
        );
        assert_eq!(
            terminal.backend().buffer().cell((3, 7)).unwrap().symbol(),
            " ",
            "a null value must leave the gauge fill empty"
        );
        capture(&terminal, "gauge-null-configured", (80, 24));

        let (negative_minimum, terminal) = render(None, true, Some("n/a"), -100.0, (80, 24));
        assert!(negative_minimum.contains("n/a (cache_hit_ratio)"));
        assert_eq!(
            terminal.backend().buffer().cell((3, 7)).unwrap().symbol(),
            " ",
            "a null value must leave the gauge empty even with a negative minimum"
        );
        capture(&terminal, "gauge-null-negative-min", (80, 24));

        let (default, terminal) = render(None, true, None, 0.0, (80, 24));
        assert!(default.contains("- (cache_hit_ratio)"), "{default}");
        assert!(!default.contains("0.00 (cache_hit_ratio)"), "{default}");
        capture(&terminal, "gauge-null-default", (80, 24));

        let (numeric, terminal) = render(Some(42.5), true, Some("n/a"), 0.0, (80, 24));
        assert!(numeric.contains("42.50 (cache_hit_ratio)"), "{numeric}");
        assert_eq!(
            terminal.backend().buffer().cell((3, 7)).unwrap().symbol(),
            "█",
            "a numeric gauge value should retain its proportional fill"
        );
        capture(&terminal, "gauge-numeric", (80, 24));

        let (no_series, terminal) = render(None, false, Some("n/a"), 0.0, (80, 24));
        assert!(no_series.contains("0.00 (No data)"), "{no_series}");
        capture(&terminal, "gauge-no-series", (80, 24));

        let (narrow, terminal) = render(None, true, Some("n/a"), 0.0, (60, 20));
        assert!(narrow.contains("n/a (cache_hit_ratio)"), "{narrow}");
        capture(&terminal, "gauge-null-narrow", (60, 20));
    }

    #[test]
    fn structurally_flat_layout_growth_uses_rendered_panel_count() {
        let mut app = test_app();
        app.panels = vec![graph_panel("CPU")];
        app.apply_layout(crate::dashboard::DashboardLayout::flat(1));
        app.panels.push(graph_panel("Memory"));
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();

        terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();

        let text = terminal_text(&terminal);
        assert!(text.contains("panels=2"));
        assert!(!text.contains("panels=1/2"));
        assert!(text.contains("CPU"));
        assert!(text.contains("Memory"));
    }

    #[test]
    fn row_search_overlay_renders_visible_row_candidate_at_narrow_viewport() {
        let mut app = nested_row_app();
        app.mode = AppMode::Search;
        app.search_query = "Expanded".to_string();
        app.search_results = vec![crate::dashboard::DashboardItemId::Row(
            crate::dashboard::RowId::new(0),
        )];
        let size = Rect::new(0, 0, 72, 24);
        let overlay = centered_rect(60, 20, size);
        let mut terminal = Terminal::new(TestBackend::new(size.width, size.height)).unwrap();

        terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();

        let text = rect_text(&terminal, overlay);
        assert!(text.contains("Search Dashboard"));
        assert!(text.contains("> Expanded"));
        assert!(text.contains("▼ Expanded"));
    }

    #[test]
    fn v2_example_fixed_grid_renders_at_supported_viewports() {
        for (width, height) in [(120, 30), (80, 24)] {
            let mut app = v2_compatibility_app();
            assert_eq!(app.panels.len(), 2);
            assert_eq!(app.selected_panel_index(), Some(0));
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
    fn export_status_stays_visible_in_a_narrow_footer() {
        let mut app = v2_compatibility_app();
        app.export_status = Some("Export failed: disk full".to_string());
        let selected = app.selected_item;
        let mut terminal = Terminal::new(TestBackend::new(52, 12)).unwrap();

        terminal.draw(|frame| draw_ui(frame, &mut app)).unwrap();

        assert!(terminal_text(&terminal).contains("Export failed: disk full"));
        assert_eq!(app.selected_item, selected);
        if let Ok(path) = std::env::var("GRAFATUI_EXPORT_STATUS_CAPTURE") {
            capture_buffer(&terminal, std::path::Path::new(&path));
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
        app.apply_layout(crate::dashboard::DashboardLayout::flat(app.panels.len()));
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

        app.selected_item = Some(crate::dashboard::DashboardItemId::Panel(1));
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
