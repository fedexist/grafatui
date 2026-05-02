use crate::app::{AppMode, AppState, PanelState, PanelType, SeriesView, ThresholdMode};
use crate::theme::Theme;
use crate::ui;
use anyhow::{Context, Result, anyhow};
use clap::ValueEnum;
use ratatui::layout::Rect;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

const CELL_WIDTH: f64 = 10.0;
const CELL_HEIGHT: f64 = 18.0;
const FONT_SIZE: f64 = 13.0;
const SMALL_FONT_SIZE: f64 = 11.0;
const PANEL_PADDING: f64 = 12.0;
const TITLE_HEIGHT: f64 = 28.0;
const X_LABEL_HEIGHT: f64 = 24.0;
const LEGEND_HEIGHT: f64 = 28.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ExportFormat {
    #[default]
    Svg,
    Png,
    Both,
}

#[derive(Debug, Clone)]
pub(crate) struct ExportOptions {
    pub(crate) dir: PathBuf,
    pub(crate) format: ExportFormat,
    pub(crate) record_max_frames: usize,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            dir: PathBuf::from("./grafatui-exports"),
            format: ExportFormat::Svg,
            record_max_frames: 300,
        }
    }
}

#[derive(Debug)]
pub(crate) struct RecordingState {
    pub(crate) dir: PathBuf,
    pub(crate) frame_count: usize,
    pub(crate) max_frames: usize,
    pub(crate) last_svg: Option<String>,
    pub(crate) frames: Vec<RecordingFrame>,
    pub(crate) started_at: String,
    pub(crate) started_instant: Instant,
    pub(crate) viewport: RecordingViewport,
}

#[derive(Debug, Serialize)]
pub(crate) struct RecordingFrame {
    pub(crate) index: usize,
    pub(crate) captured_at: String,
    pub(crate) elapsed_ms: u128,
    pub(crate) files: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RecordingViewport {
    width: u16,
    height: u16,
}

#[derive(Debug, Serialize)]
struct RecordingManifest {
    version: u8,
    started_at: String,
    completed_at: String,
    format: ExportFormat,
    viewport: RecordingViewport,
    changed_only: bool,
    frames: Vec<RecordingFrame>,
}

#[derive(Clone, Copy)]
struct PlotRect {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

struct LineStyle<'a> {
    color: &'a str,
    dash: Option<&'a str>,
    width: f64,
}

impl PlotRect {
    fn right(self) -> f64 {
        self.left + self.width
    }

    fn bottom(self) -> f64 {
        self.top + self.height
    }
}

pub(crate) fn export_current(app: &mut AppState, viewport: Rect) -> Result<Vec<PathBuf>> {
    let svg = render_svg(app, viewport);
    let stem = format!("grafatui-{}", timestamp_id());
    let paths = write_outputs(&svg, &app.export.dir, &stem, app.export.format)?;
    app.export_status = Some(format!("Exported {}", display_paths(&paths)));
    Ok(paths)
}

pub(crate) fn toggle_recording(app: &mut AppState, viewport: Rect) -> Result<()> {
    if app.recording.is_some() {
        stop_recording(app)
    } else {
        start_recording(app, viewport)
    }
}

pub(crate) fn capture_recording_frame(app: &mut AppState, viewport: Rect) -> Result<()> {
    let Some(recording) = app.recording.as_ref() else {
        return Ok(());
    };
    if recording.frame_count >= recording.max_frames {
        app.export_status = Some(format!(
            "Recording capped at {} frames",
            recording.max_frames
        ));
        return Ok(());
    }

    let svg = render_svg(app, viewport);
    if recording.last_svg.as_deref() == Some(svg.as_str()) {
        return Ok(());
    }

    let frame_index = recording.frame_count + 1;
    let stem = format!("frame-{frame_index:06}");
    let paths = write_outputs(&svg, &recording.dir, &stem, app.export.format)?;
    let captured_at = timestamp_rfc3339();
    let elapsed_ms = recording.started_instant.elapsed().as_millis();
    let files = paths
        .iter()
        .filter_map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .collect::<Vec<_>>();

    if let Some(recording) = app.recording.as_mut() {
        recording.frame_count = frame_index;
        recording.last_svg = Some(svg);
        recording.frames.push(RecordingFrame {
            index: frame_index,
            captured_at,
            elapsed_ms,
            files,
        });
    }
    app.export_status = Some(format!("Recording frame {frame_index}"));
    Ok(())
}

fn start_recording(app: &mut AppState, viewport: Rect) -> Result<()> {
    let started_at = timestamp_rfc3339();
    let dir = app
        .export
        .dir
        .join(format!("grafatui-recording-{}", timestamp_id()));
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create recording directory {}", dir.display()))?;
    app.recording = Some(RecordingState {
        dir,
        frame_count: 0,
        max_frames: app.export.record_max_frames,
        last_svg: None,
        frames: Vec::new(),
        started_at,
        started_instant: Instant::now(),
        viewport: RecordingViewport {
            width: viewport.width,
            height: viewport.height,
        },
    });
    app.export_status = Some("Recording started".to_string());
    capture_recording_frame(app, viewport)
}

fn stop_recording(app: &mut AppState) -> Result<()> {
    let Some(recording) = app.recording.take() else {
        return Ok(());
    };

    let manifest = RecordingManifest {
        version: 1,
        started_at: recording.started_at,
        completed_at: timestamp_rfc3339(),
        format: app.export.format,
        viewport: recording.viewport,
        changed_only: true,
        frames: recording.frames,
    };
    let manifest_path = recording.dir.join("manifest.json");
    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, json)
        .with_context(|| format!("failed to write {}", manifest_path.display()))?;
    app.export_status = Some(format!(
        "Recording saved {} ({} frames)",
        recording.dir.display(),
        recording.frame_count
    ));
    Ok(())
}

pub(crate) fn render_svg(app: &AppState, viewport: Rect) -> String {
    let width = f64::from(viewport.width).max(1.0) * CELL_WIDTH;
    let height = f64::from(viewport.height).max(1.0) * CELL_HEIGHT;
    let bg = color_hex(app.theme.background, "#111111");
    let text = color_hex(app.theme.text, "#e6e6e6");
    let border = color_hex(app.theme.border, "#555555");

    let mut out = String::new();
    write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}">"#
    )
    .unwrap();
    write!(out, r#"<rect width="100%" height="100%" fill="{bg}"/>"#).unwrap();
    write!(
        out,
        r#"<g font-family="ui-monospace, SFMono-Regular, Menlo, Consolas, monospace" font-size="{FONT_SIZE}" fill="{text}">"#
    )
    .unwrap();
    render_header(app, &mut out, width, &text, &border);

    for (rect, index) in ui::visible_panel_rects(viewport, app) {
        let Some(panel) = app.panels.get(index) else {
            continue;
        };
        let selected = index == app.selected_panel;
        let panel_rect = scaled_rect(rect);
        render_panel(app, panel, panel_rect, selected, &mut out);
    }

    render_footer(app, &mut out, width, height, &text, &border);
    out.push_str("</g></svg>");
    out
}

fn render_header(app: &AppState, out: &mut String, width: f64, text: &str, border: &str) {
    write!(
        out,
        r#"<rect x="4" y="4" width="{:.0}" height="44" fill="none" stroke="{border}"/>"#,
        width - 8.0
    )
    .unwrap();
    let title = format!(
        "{} - range={} step={} panels={}",
        app.title,
        humantime::format_duration(app.range),
        humantime::format_duration(app.step),
        app.panels.len()
    );
    write_text(out, width / 2.0, 31.0, &title, text, "middle", FONT_SIZE);
}

fn render_footer(
    app: &AppState,
    out: &mut String,
    width: f64,
    height: f64,
    text: &str,
    border: &str,
) {
    write!(
        out,
        r#"<line x1="4" y1="{:.0}" x2="{:.0}" y2="{:.0}" stroke="{border}"/>"#,
        height - 30.0,
        width - 4.0,
        height - 30.0
    )
    .unwrap();
    let mode = match app.mode {
        AppMode::Normal => "NORMAL",
        AppMode::Search => "SEARCH",
        AppMode::Fullscreen => "FULLSCREEN",
        AppMode::Inspect => "INSPECT",
        AppMode::FullscreenInspect => "FULLSCREEN INSPECT",
    };
    let recording = if app.recording.is_some() {
        " | REC"
    } else {
        ""
    };
    write_text(
        out,
        10.0,
        height - 11.0,
        &format!("Mode: {mode}{recording}"),
        text,
        "start",
        SMALL_FONT_SIZE,
    );
}

fn render_panel(
    app: &AppState,
    panel: &PanelState,
    rect: PlotRect,
    selected: bool,
    out: &mut String,
) {
    let theme = &app.theme;
    let border = if selected {
        color_hex(theme.border_selected, "#f0d000")
    } else {
        color_hex(theme.border, "#555555")
    };
    let title = color_hex(theme.title, "#00c8ff");
    let bg = color_hex(theme.background, "#111111");

    write!(
        out,
        r#"<rect x="{:.0}" y="{:.0}" width="{:.0}" height="{:.0}" fill="{bg}" stroke="{border}"/>"#,
        rect.left, rect.top, rect.width, rect.height
    )
    .unwrap();
    write_text(
        out,
        rect.left + 8.0,
        rect.top + 18.0,
        &panel.title,
        &title,
        "start",
        FONT_SIZE,
    );

    let inner = PlotRect {
        left: rect.left + PANEL_PADDING,
        top: rect.top + TITLE_HEIGHT,
        width: (rect.width - PANEL_PADDING * 2.0).max(0.0),
        height: (rect.height - TITLE_HEIGHT - PANEL_PADDING).max(0.0),
    };

    if let Some(err) = &panel.last_error {
        write_text(
            out,
            inner.left,
            inner.top + 18.0,
            err,
            &color_hex(Color::Red, "#ff5555"),
            "start",
            FONT_SIZE,
        );
        return;
    }

    match panel.panel_type {
        PanelType::Graph | PanelType::Unknown => render_graph_panel(app, panel, inner, out),
        PanelType::Stat => render_stat_panel(app, panel, inner, out),
        PanelType::Gauge => render_gauge_panel(app, panel, inner, out),
        PanelType::BarGauge => render_bar_gauge_panel(app, panel, inner, out),
        PanelType::Table => render_table_panel(app, panel, inner, out),
        PanelType::Heatmap => render_heatmap_panel(app, panel, inner, out),
    }
}

fn render_graph_panel(app: &AppState, panel: &PanelState, rect: PlotRect, out: &mut String) {
    if rect.width < 120.0 || rect.height < 80.0 {
        return;
    }

    let legend_height = if panel.series.is_empty() {
        0.0
    } else {
        LEGEND_HEIGHT
    };
    let y_label_width = 64.0;
    let plot = PlotRect {
        left: rect.left + y_label_width,
        top: rect.top + 6.0,
        width: (rect.width - y_label_width - 8.0).max(1.0),
        height: (rect.height - X_LABEL_HEIGHT - legend_height - 10.0).max(1.0),
    };

    let (x_min, x_max) = app.time_bounds();
    let y_bounds = ui::calculate_y_bounds(panel);
    let text = color_hex(app.theme.text, "#e6e6e6");
    let axis = color_hex(Color::Gray, "#777777");
    let grid = "#6d6d6d";

    write!(
        out,
        r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{axis}"/>"#,
        plot.left,
        plot.top,
        plot.left,
        plot.bottom()
    )
    .unwrap();
    write!(
        out,
        r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{axis}"/>"#,
        plot.left,
        plot.bottom(),
        plot.right(),
        plot.bottom()
    )
    .unwrap();

    for tick in value_ticks(y_bounds[0], y_bounds[1]) {
        let y = map_y(tick, y_bounds, plot);
        draw_line(
            out,
            (plot.left, y),
            (plot.right(), y),
            LineStyle {
                color: grid,
                dash: Some("3 5"),
                width: 0.7,
            },
        );
        write_text(
            out,
            plot.left - 8.0,
            y + 4.0,
            &ui::format_si(tick),
            grid,
            "end",
            SMALL_FONT_SIZE,
        );
    }

    for tick in time_ticks(x_min, x_max) {
        let x = map_x(tick, [x_min, x_max], plot);
        draw_line(
            out,
            (x, plot.top),
            (x, plot.bottom()),
            LineStyle {
                color: grid,
                dash: Some("3 5"),
                width: 0.7,
            },
        );
        write_text(
            out,
            x,
            plot.bottom() + 17.0,
            &ui::format_time(tick),
            grid,
            "middle",
            SMALL_FONT_SIZE,
        );
    }

    write_text(
        out,
        plot.left - 8.0,
        plot.bottom() + 4.0,
        &ui::format_si(y_bounds[0]),
        &text,
        "end",
        SMALL_FONT_SIZE,
    );
    write_text(
        out,
        plot.left - 8.0,
        plot.top + 4.0,
        &ui::format_si(y_bounds[1]),
        &text,
        "end",
        SMALL_FONT_SIZE,
    );
    write_text(
        out,
        plot.left,
        plot.bottom() + 17.0,
        &ui::format_time(x_min),
        &text,
        "start",
        SMALL_FONT_SIZE,
    );
    write_text(
        out,
        plot.right(),
        plot.bottom() + 17.0,
        &ui::format_time(x_max),
        &text,
        "end",
        SMALL_FONT_SIZE,
    );

    for (value, color, dashed) in threshold_lines(panel, app) {
        if value <= y_bounds[0] || value >= y_bounds[1] {
            continue;
        }
        let y = map_y(value, y_bounds, plot);
        let color = color_hex(color, "#ffaa00");
        draw_line(
            out,
            (plot.left, y),
            (plot.right(), y),
            LineStyle {
                color: &color,
                dash: dashed.then_some("6 5"),
                width: 1.2,
            },
        );
        write_text(
            out,
            plot.left - 8.0,
            y + 4.0,
            &ui::format_si(value),
            &color,
            "end",
            SMALL_FONT_SIZE,
        );
    }

    if let Some(cursor_x) = app.cursor_x {
        if cursor_x >= x_min && cursor_x <= x_max {
            let x = map_x(cursor_x, [x_min, x_max], plot);
            draw_line(
                out,
                (x, plot.top),
                (x, plot.bottom()),
                LineStyle {
                    color: "#ffffff",
                    dash: Some("4 4"),
                    width: 1.0,
                },
            );
        }
    }

    for (index, series) in panel.series.iter().enumerate() {
        if !series.visible {
            continue;
        }
        let color = series_color(panel, &app.theme, index);
        let color = color_hex(color, "#00ff88");
        if let Some(path) = series_path(series, [x_min, x_max], y_bounds, plot) {
            write!(
                out,
                r#"<path d="{path}" fill="none" stroke="{color}" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round"/>"#
            )
            .unwrap();
        }
    }

    render_legend(
        app,
        panel,
        plot.left,
        plot.bottom() + X_LABEL_HEIGHT,
        plot.width,
        out,
    );
}

fn render_stat_panel(app: &AppState, panel: &PanelState, rect: PlotRect, out: &mut String) {
    let Some((series, value)) = first_visible_value(panel) else {
        render_no_data(app, rect, out);
        return;
    };

    let color = value_color(app, panel, value);
    let text = color_hex(app.theme.text, "#e6e6e6");
    write_text(
        out,
        rect.left + rect.width / 2.0,
        rect.top + 34.0,
        &ui::format_si(value),
        &color,
        "middle",
        28.0,
    );
    write_text(
        out,
        rect.left + rect.width / 2.0,
        rect.top + 56.0,
        &series.name,
        &text,
        "middle",
        SMALL_FONT_SIZE,
    );

    let sparkline = PlotRect {
        left: rect.left + 8.0,
        top: rect.top + 72.0,
        width: (rect.width - 16.0).max(1.0),
        height: (rect.height - 82.0).max(1.0),
    };
    render_sparkline(series, sparkline, &color, out);
}

fn render_gauge_panel(app: &AppState, panel: &PanelState, rect: PlotRect, out: &mut String) {
    let Some((series, value)) = first_visible_value(panel) else {
        render_no_data(app, rect, out);
        return;
    };

    let min = panel.min.unwrap_or(0.0);
    let max = panel
        .max
        .unwrap_or(if value > 100.0 { value * 1.2 } else { 100.0 });
    let ratio = value_ratio(value, min, max);
    let color = value_color(app, panel, value);
    let text = color_hex(app.theme.text, "#e6e6e6");
    let track = color_hex(Color::DarkGray, "#444444");
    let gauge = PlotRect {
        left: rect.left + 10.0,
        top: rect.top + rect.height / 2.0 - 13.0,
        width: (rect.width - 20.0).max(1.0),
        height: 26.0,
    };

    write_rect(out, gauge, &track, "none", 0.0);
    let fill = PlotRect {
        width: gauge.width * ratio,
        ..gauge
    };
    write_rect(out, fill, &color, "none", 0.0);
    write_text(
        out,
        rect.left + rect.width / 2.0,
        gauge.top - 12.0,
        &series.name,
        &text,
        "middle",
        SMALL_FONT_SIZE,
    );
    write_text(
        out,
        rect.left + rect.width / 2.0,
        gauge.top + 18.0,
        &format!("{} ({:.0}%)", ui::format_si(value), ratio * 100.0),
        "#ffffff",
        "middle",
        FONT_SIZE,
    );
    write_text(
        out,
        gauge.left,
        gauge.bottom() + 17.0,
        &ui::format_si(min),
        &text,
        "start",
        SMALL_FONT_SIZE,
    );
    write_text(
        out,
        gauge.right(),
        gauge.bottom() + 17.0,
        &ui::format_si(max),
        &text,
        "end",
        SMALL_FONT_SIZE,
    );
}

fn render_bar_gauge_panel(app: &AppState, panel: &PanelState, rect: PlotRect, out: &mut String) {
    let mut values = panel
        .series
        .iter()
        .filter(|series| series.visible)
        .filter_map(|series| series.value.map(|value| (series, value)))
        .collect::<Vec<_>>();
    values.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    if values.is_empty() {
        render_no_data(app, rect, out);
        return;
    }

    let max_value = values
        .iter()
        .filter_map(|(_, value)| value.is_finite().then_some(*value))
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let row_height = 22.0;
    let max_rows = (rect.height / row_height).floor().max(1.0) as usize;
    let label_width = (rect.width * 0.32).clamp(80.0, 180.0);
    let bar_width = (rect.width - label_width - 76.0).max(1.0);
    let text = color_hex(app.theme.text, "#e6e6e6");
    let track = color_hex(Color::DarkGray, "#444444");

    for (row, (series, value)) in values.into_iter().take(max_rows).enumerate() {
        let y = rect.top + row as f64 * row_height + 15.0;
        let ratio = (value / max_value).clamp(0.0, 1.0);
        let color = value_color(app, panel, value);
        write_text(
            out,
            rect.left + 4.0,
            y,
            &series.name,
            &text,
            "start",
            SMALL_FONT_SIZE,
        );
        let track_rect = PlotRect {
            left: rect.left + label_width,
            top: y - 12.0,
            width: bar_width,
            height: 14.0,
        };
        write_rect(out, track_rect, &track, "none", 0.0);
        write_rect(
            out,
            PlotRect {
                width: track_rect.width * ratio,
                ..track_rect
            },
            &color,
            "none",
            0.0,
        );
        write_text(
            out,
            track_rect.right() + 8.0,
            y,
            &ui::format_si(value),
            &color,
            "start",
            SMALL_FONT_SIZE,
        );
    }
}

fn render_table_panel(app: &AppState, panel: &PanelState, rect: PlotRect, out: &mut String) {
    let values = panel
        .series
        .iter()
        .filter(|series| series.visible)
        .collect::<Vec<_>>();
    if values.is_empty() {
        render_no_data(app, rect, out);
        return;
    }

    let text = color_hex(app.theme.text, "#e6e6e6");
    let title = color_hex(app.theme.title, "#00c8ff");
    let border = color_hex(app.theme.border, "#555555");
    let row_height = 20.0;
    let value_x = rect.left + rect.width * 0.7;
    let max_rows = ((rect.height - row_height) / row_height).floor().max(1.0) as usize;

    write_text(
        out,
        rect.left + 6.0,
        rect.top + 15.0,
        "Series",
        &title,
        "start",
        SMALL_FONT_SIZE,
    );
    write_text(
        out,
        value_x,
        rect.top + 15.0,
        "Value",
        &title,
        "start",
        SMALL_FONT_SIZE,
    );
    draw_line(
        out,
        (rect.left, rect.top + row_height),
        (rect.right(), rect.top + row_height),
        LineStyle {
            color: &border,
            dash: None,
            width: 0.8,
        },
    );

    for (row, series) in values.into_iter().take(max_rows).enumerate() {
        let y = rect.top + row_height * (row as f64 + 2.0) - 5.0;
        let value = series
            .value
            .map(ui::format_si)
            .unwrap_or_else(|| "-".to_string());
        let value_color = series
            .value
            .map(|value| value_color(app, panel, value))
            .unwrap_or_else(|| text.clone());
        write_text(
            out,
            rect.left + 6.0,
            y,
            &series.name,
            &text,
            "start",
            SMALL_FONT_SIZE,
        );
        write_text(
            out,
            value_x,
            y,
            &value,
            &value_color,
            "start",
            SMALL_FONT_SIZE,
        );
    }
}

fn render_heatmap_panel(app: &AppState, panel: &PanelState, rect: PlotRect, out: &mut String) {
    let visible = panel
        .series
        .iter()
        .filter(|series| series.visible)
        .collect::<Vec<_>>();
    if visible.is_empty() {
        render_no_data(app, rect, out);
        return;
    }

    let (mut min, mut max) = (f64::MAX, f64::MIN);
    for series in &visible {
        for (_, value) in &series.points {
            if value.is_finite() {
                min = min.min(*value);
                max = max.max(*value);
            }
        }
    }
    if !min.is_finite() || !max.is_finite() || min == max {
        render_no_data(app, rect, out);
        return;
    }

    let label_width = (rect.width * 0.24).clamp(60.0, 150.0);
    let plot_left = rect.left + label_width;
    let plot_width = (rect.width - label_width - 4.0).max(1.0);
    let rows = visible
        .len()
        .min((rect.height / 18.0).floor().max(1.0) as usize);
    let row_height = (rect.height / rows as f64).max(1.0);
    let cols = (plot_width / 10.0).floor().max(1.0) as usize;
    let cell_width = plot_width / cols as f64;
    let text = color_hex(app.theme.text, "#e6e6e6");

    for (row, series) in visible.into_iter().take(rows).enumerate() {
        let top = rect.top + row as f64 * row_height;
        write_text(
            out,
            rect.left + 4.0,
            top + row_height.min(16.0),
            &series.name,
            &text,
            "start",
            SMALL_FONT_SIZE,
        );
        if series.points.is_empty() {
            continue;
        }
        let step = (series.points.len() as f64 / cols as f64).max(1.0);
        for col in 0..cols {
            let point = ((col as f64 * step) as usize).min(series.points.len() - 1);
            let value = series.points[point].1;
            let color = if value.is_finite() {
                let normalized = ((value - min) / (max - min)).clamp(0.0, 1.0);
                color_hex(ui::value_to_heatmap_color(normalized), "#666666")
            } else {
                color_hex(Color::DarkGray, "#444444")
            };
            write_rect(
                out,
                PlotRect {
                    left: plot_left + col as f64 * cell_width,
                    top,
                    width: cell_width.max(1.0),
                    height: (row_height - 2.0).max(1.0),
                },
                &color,
                "none",
                0.0,
            );
        }
    }
}

fn first_visible_value(panel: &PanelState) -> Option<(&SeriesView, f64)> {
    panel
        .series
        .iter()
        .filter(|series| series.visible)
        .find_map(|series| series.value.map(|value| (series, value)))
}

fn value_color(app: &AppState, panel: &PanelState, value: f64) -> String {
    color_hex(
        panel
            .get_color_for_value(value)
            .unwrap_or(app.theme.palette[0]),
        "#00ff88",
    )
}

fn value_ratio(value: f64, min: f64, max: f64) -> f64 {
    if !value.is_finite() || max <= min {
        return 0.0;
    }
    ((value - min) / (max - min)).clamp(0.0, 1.0)
}

fn render_no_data(app: &AppState, rect: PlotRect, out: &mut String) {
    write_text(
        out,
        rect.left + 8.0,
        rect.top + 24.0,
        "No data",
        &color_hex(app.theme.text, "#e6e6e6"),
        "start",
        FONT_SIZE,
    );
}

fn render_sparkline(series: &SeriesView, rect: PlotRect, color: &str, out: &mut String) {
    if rect.width <= 0.0 || rect.height <= 0.0 || series.points.len() < 2 {
        return;
    }

    let values = series
        .points
        .iter()
        .filter_map(|(_, value)| value.is_finite().then_some(*value))
        .collect::<Vec<_>>();
    if values.len() < 2 {
        return;
    }

    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = (max - min).max(f64::EPSILON);
    let step = rect.width / (values.len() - 1) as f64;
    let mut path = String::new();
    for (index, value) in values.iter().enumerate() {
        let x = rect.left + index as f64 * step;
        let y = rect.bottom() - ((*value - min) / span).clamp(0.0, 1.0) * rect.height;
        if index == 0 {
            write!(path, "M {x:.2} {y:.2}").unwrap();
        } else {
            write!(path, " L {x:.2} {y:.2}").unwrap();
        }
    }

    write!(
        out,
        r#"<path d="{path}" fill="none" stroke="{color}" stroke-width="1.4" stroke-linejoin="round" stroke-linecap="round"/>"#
    )
    .unwrap();
}

fn render_legend(
    app: &AppState,
    panel: &PanelState,
    left: f64,
    top: f64,
    width: f64,
    out: &mut String,
) {
    let mut x = left;
    let mut y = top + 15.0;
    let text = color_hex(app.theme.text, "#e6e6e6");
    let cursor_values = cursor_values(panel, app);

    for (index, series) in panel.series.iter().enumerate().filter(|(_, s)| s.visible) {
        let color = color_hex(series_color(panel, &app.theme, index), "#00ff88");
        let value = cursor_values
            .get(&series.name)
            .copied()
            .or(series.value)
            .map(ui::format_si);
        let label = value
            .map(|value| format!("{} ({value})", series.name))
            .unwrap_or_else(|| series.name.clone());
        let estimated_width = (label.len() as f64 * 7.0) + 24.0;
        if x + estimated_width > left + width && x > left {
            x = left;
            y += 15.0;
        }
        write!(
            out,
            r#"<rect x="{:.2}" y="{:.2}" width="8" height="8" fill="{color}"/>"#,
            x,
            y - 8.0
        )
        .unwrap();
        write_text(out, x + 13.0, y, &label, &text, "start", SMALL_FONT_SIZE);
        x += estimated_width;
    }
}

fn cursor_values(panel: &PanelState, app: &AppState) -> std::collections::HashMap<String, f64> {
    let mut values = std::collections::HashMap::new();
    let Some(cursor_x) = app.cursor_x else {
        return values;
    };

    for series in &panel.series {
        let closest = series.points.iter().min_by(|a, b| {
            let da = (a.0 - cursor_x).abs();
            let db = (b.0 - cursor_x).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
        if let Some((ts, value)) = closest {
            if (ts - cursor_x).abs() <= app.step.as_secs_f64() * 2.0 {
                values.insert(series.name.clone(), *value);
            }
        }
    }
    values
}

fn series_path(
    series: &SeriesView,
    x_bounds: [f64; 2],
    y_bounds: [f64; 2],
    plot: PlotRect,
) -> Option<String> {
    let mut path = String::new();
    let mut started = false;

    for &(x_value, y_value) in &series.points {
        if !x_value.is_finite()
            || !y_value.is_finite()
            || x_value < x_bounds[0]
            || x_value > x_bounds[1]
        {
            continue;
        }
        let x = map_x(x_value, x_bounds, plot);
        let y = map_y(y_value, y_bounds, plot);
        if started {
            write!(path, " L {x:.2} {y:.2}").unwrap();
        } else {
            write!(path, "M {x:.2} {y:.2}").unwrap();
            started = true;
        }
    }

    started.then_some(path)
}

fn threshold_lines(panel: &PanelState, app: &AppState) -> Vec<(f64, Color, bool)> {
    let Some(thresholds) = &panel.thresholds else {
        return Vec::new();
    };
    thresholds
        .steps
        .iter()
        .filter_map(|step| {
            let value = step.value?;
            let value = match thresholds.mode {
                ThresholdMode::Absolute => value,
                ThresholdMode::Percentage => {
                    let min = panel.min.unwrap_or(0.0);
                    let max = panel.max.unwrap_or(100.0);
                    min + (value / 100.0) * (max - min)
                }
            };
            let dashed = app.threshold_marker.starts_with("dashed")
                || thresholds.style.as_deref() == Some("dashed");
            Some((value, step.color, dashed))
        })
        .collect()
}

fn series_color(panel: &PanelState, theme: &Theme, index: usize) -> Color {
    if panel.series.len() > theme.palette.len() {
        ui::get_hash_color(&panel.series[index].name)
    } else {
        theme.palette[index % theme.palette.len()]
    }
}

fn write_outputs(svg: &str, dir: &Path, stem: &str, format: ExportFormat) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(dir)
        .with_context(|| format!("failed to create export directory {}", dir.display()))?;
    let mut paths = Vec::new();

    if matches!(format, ExportFormat::Svg | ExportFormat::Both) {
        let path = dir.join(format!("{stem}.svg"));
        fs::write(&path, svg).with_context(|| format!("failed to write {}", path.display()))?;
        paths.push(path);
    }

    if matches!(format, ExportFormat::Png | ExportFormat::Both) {
        let path = dir.join(format!("{stem}.png"));
        write_png(svg, &path)?;
        paths.push(path);
    }

    Ok(paths)
}

fn write_png(svg: &str, path: &Path) -> Result<()> {
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_data(svg.as_bytes(), &options)
        .map_err(|err| anyhow!("failed to parse generated SVG: {err}"))?;
    let size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .context("failed to allocate PNG pixmap")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );
    pixmap
        .save_png(path)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn draw_line(out: &mut String, start: (f64, f64), end: (f64, f64), style: LineStyle<'_>) {
    let (x1, y1) = start;
    let (x2, y2) = end;
    write!(
        out,
        r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="{}" stroke-width="{:.2}""#,
        style.color,
        style.width
    )
    .unwrap();
    if let Some(dash) = style.dash {
        write!(out, r#" stroke-dasharray="{dash}""#).unwrap();
    }
    out.push_str("/>");
}

fn write_text(out: &mut String, x: f64, y: f64, text: &str, color: &str, anchor: &str, size: f64) {
    write!(
        out,
        r#"<text x="{x:.2}" y="{y:.2}" fill="{color}" font-size="{size:.1}" text-anchor="{anchor}">{}</text>"#,
        escape_xml(text)
    )
    .unwrap();
}

fn write_rect(out: &mut String, rect: PlotRect, fill: &str, stroke: &str, stroke_width: f64) {
    write!(
        out,
        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{fill}" stroke="{stroke}" stroke-width="{stroke_width:.2}"/>"#,
        rect.left, rect.top, rect.width, rect.height
    )
    .unwrap();
}

fn scaled_rect(rect: Rect) -> PlotRect {
    PlotRect {
        left: f64::from(rect.x) * CELL_WIDTH,
        top: f64::from(rect.y) * CELL_HEIGHT,
        width: f64::from(rect.width) * CELL_WIDTH,
        height: f64::from(rect.height) * CELL_HEIGHT,
    }
}

fn map_x(value: f64, bounds: [f64; 2], plot: PlotRect) -> f64 {
    let span = (bounds[1] - bounds[0]).max(f64::EPSILON);
    plot.left + ((value - bounds[0]) / span).clamp(0.0, 1.0) * plot.width
}

fn map_y(value: f64, bounds: [f64; 2], plot: PlotRect) -> f64 {
    let span = (bounds[1] - bounds[0]).max(f64::EPSILON);
    plot.bottom() - ((value - bounds[0]) / span).clamp(0.0, 1.0) * plot.height
}

fn value_ticks(min: f64, max: f64) -> Vec<f64> {
    if !min.is_finite() || !max.is_finite() || max <= min {
        return Vec::new();
    }

    let step = nice_step((max - min) / 3.0);
    let mut tick = (min / step).ceil() * step;
    let mut ticks = Vec::new();

    while tick < max {
        if tick > min {
            ticks.push(tick);
        }
        tick += step;
    }
    ticks
}

fn nice_step(raw: f64) -> f64 {
    let exponent = raw.abs().log10().floor();
    let base = 10f64.powf(exponent);
    let fraction = raw / base;
    let nice = if fraction <= 1.0 {
        1.0
    } else if fraction <= 2.0 {
        2.0
    } else if fraction <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * base
}

fn time_ticks(start: f64, end: f64) -> Vec<f64> {
    if !start.is_finite() || !end.is_finite() || end <= start {
        return Vec::new();
    }

    let range = end - start;
    let step = if range <= 10.0 * 60.0 {
        60.0
    } else if range <= 90.0 * 60.0 {
        30.0 * 60.0
    } else if range <= 6.0 * 3600.0 {
        3600.0
    } else if range <= 24.0 * 3600.0 {
        6.0 * 3600.0
    } else {
        24.0 * 3600.0
    };

    let mut tick = (start / step).ceil() * step;
    let mut ticks = Vec::new();
    while tick < end {
        if tick > start {
            ticks.push(tick);
        }
        tick += step;
    }
    ticks
}

fn color_hex(color: Color, reset: &str) -> String {
    match color {
        Color::Reset => reset.to_string(),
        Color::Black => "#000000".to_string(),
        Color::Red => "#cc3333".to_string(),
        Color::Green => "#33cc66".to_string(),
        Color::Yellow => "#d6c343".to_string(),
        Color::Blue => "#4f83ff".to_string(),
        Color::Magenta => "#cc66cc".to_string(),
        Color::Cyan => "#33c8cc".to_string(),
        Color::Gray => "#a0a0a0".to_string(),
        Color::DarkGray => "#666666".to_string(),
        Color::LightRed => "#ff6666".to_string(),
        Color::LightGreen => "#66ff99".to_string(),
        Color::LightYellow => "#fff06a".to_string(),
        Color::LightBlue => "#7aa2ff".to_string(),
        Color::LightMagenta => "#ff8cff".to_string(),
        Color::LightCyan => "#66ffff".to_string(),
        Color::White => "#f5f5f5".to_string(),
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Indexed(value) => indexed_color_hex(value).to_string(),
    }
}

fn indexed_color_hex(value: u8) -> &'static str {
    const ANSI: [&str; 16] = [
        "#000000", "#cc3333", "#33cc66", "#d6c343", "#4f83ff", "#cc66cc", "#33c8cc", "#d0d0d0",
        "#666666", "#ff6666", "#66ff99", "#fff06a", "#7aa2ff", "#ff8cff", "#66ffff", "#f5f5f5",
    ];
    ANSI.get(value as usize).copied().unwrap_or("#a0a0a0")
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn timestamp_id() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S-%9f").to_string()
}

fn timestamp_rfc3339() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn display_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{PanelState, SeriesView, YAxisMode};

    fn test_panel(start: f64) -> PanelState {
        PanelState {
            title: "CPU <main>".to_string(),
            exprs: vec![],
            legends: vec![],
            series: vec![SeriesView {
                name: "usage & total".to_string(),
                value: Some(10.0),
                points: vec![(start, 0.0), (start + 50.0, 50.0), (start + 100.0, 100.0)],
                visible: true,
            }],
            last_error: None,
            last_url: None,
            last_samples: 3,
            grid: None,
            y_axis_mode: YAxisMode::Auto,
            panel_type: PanelType::Graph,
            thresholds: None,
            min: None,
            max: None,
            autogrid: None,
        }
    }

    fn test_export_dir(name: &str) -> PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("grafatui-{name}-{}-{suffix}", std::process::id()))
    }

    fn test_app(export: ExportOptions) -> AppState {
        let prom = crate::prom::PromClient::new("http://localhost:9090".to_string());
        let now = chrono::Utc::now().timestamp() as f64;
        let range = std::time::Duration::from_secs(100);
        AppState::new(
            prom,
            range,
            std::time::Duration::from_secs(10),
            std::time::Duration::from_secs(1),
            "Dash & Main".to_string(),
            vec![test_panel(now - range.as_secs_f64())],
            0,
            Theme::default(),
            "dashed-line".to_string(),
            export,
        )
    }

    fn test_app_with_panel_type(panel_type: PanelType) -> AppState {
        let mut app = test_app(ExportOptions::default());
        app.panels[0].panel_type = panel_type;
        app
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("<a&b\"c'>"), "&lt;a&amp;b&quot;c&apos;&gt;");
    }

    #[test]
    fn test_map_coordinates_respect_bounds() {
        let plot = PlotRect {
            left: 10.0,
            top: 20.0,
            width: 100.0,
            height: 50.0,
        };
        assert_eq!(map_x(50.0, [0.0, 100.0], plot), 60.0);
        assert_eq!(map_y(50.0, [0.0, 100.0], plot), 45.0);
    }

    #[test]
    fn test_value_ticks_are_interior() {
        let ticks = value_ticks(329.0, 1287.0);
        assert!(ticks.contains(&500.0));
        assert!(ticks.contains(&1000.0));
        assert!(!ticks.contains(&329.0));
        assert!(!ticks.contains(&1287.0));
    }

    #[test]
    fn test_time_ticks_choose_expected_boundaries() {
        let two_hours = time_ticks(11.0 * 3600.0 + 22.0 * 60.0, 13.0 * 3600.0 + 22.0 * 60.0);
        assert_eq!(two_hours, vec![12.0 * 3600.0, 13.0 * 3600.0]);

        let one_hour = time_ticks(12.0 * 3600.0 + 22.0 * 60.0, 13.0 * 3600.0 + 22.0 * 60.0);
        assert_eq!(one_hour, vec![12.5 * 3600.0, 13.0 * 3600.0]);

        let five_minutes = time_ticks(12.0 * 3600.0 + 22.0 * 60.0, 12.0 * 3600.0 + 27.0 * 60.0);
        assert_eq!(
            five_minutes,
            vec![
                12.0 * 3600.0 + 23.0 * 60.0,
                12.0 * 3600.0 + 24.0 * 60.0,
                12.0 * 3600.0 + 25.0 * 60.0,
                12.0 * 3600.0 + 26.0 * 60.0
            ]
        );
    }

    #[test]
    fn test_svg_contains_escaped_text_and_axes() {
        let app = test_app(ExportOptions::default());

        let svg = render_svg(&app, Rect::new(0, 0, 100, 40));
        assert!(svg.starts_with("<svg "));
        assert!(svg.contains("Dash &amp; Main"));
        assert!(svg.contains("CPU &lt;main&gt;"));
        assert!(svg.contains("<line "));
        assert!(svg.contains("<path "));
    }

    #[test]
    fn test_graph_export_uses_refreshed_time_bounds() {
        let mut app = test_app(ExportOptions::default());
        app.view_end_ts = 1_700_000_000;
        app.range = std::time::Duration::from_secs(100);
        app.panels[0].series[0].points = vec![
            (1_699_999_900.0, 0.0),
            (1_699_999_950.0, 50.0),
            (1_700_000_000.0, 100.0),
        ];

        let svg = render_svg(&app, Rect::new(0, 0, 100, 40));

        assert!(svg.contains(&ui::format_time(1_699_999_900.0)));
        assert!(svg.contains(&ui::format_time(1_700_000_000.0)));
    }

    #[test]
    fn test_non_graph_panel_exports_render_representative_svg() {
        let cases = [
            (PanelType::Stat, "usage &amp; total", "<path "),
            (PanelType::Gauge, "10%", "100.00"),
            (PanelType::BarGauge, "usage &amp; total", "10.00"),
            (PanelType::Table, "Series", "Value"),
            (PanelType::Heatmap, "#cc3333", "#33c8cc"),
        ];

        for (panel_type, first, second) in cases {
            let app = test_app_with_panel_type(panel_type);
            let svg = render_svg(&app, Rect::new(0, 0, 100, 40));

            assert!(svg.contains("CPU &lt;main&gt;"));
            assert!(svg.contains(first), "{panel_type:?} missing {first}");
            assert!(svg.contains(second), "{panel_type:?} missing {second}");
            assert!(!svg.contains("No data"));
        }
    }

    #[test]
    fn test_png_rasterization_writes_non_empty_file() {
        let app = test_app(ExportOptions::default());
        let svg = render_svg(&app, Rect::new(0, 0, 100, 40));
        let dir = test_export_dir("png");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("snapshot.png");

        write_png(&svg, &path).unwrap();

        assert!(fs::metadata(&path).unwrap().len() > 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_recording_writes_manifest_and_skips_duplicates() {
        let dir = test_export_dir("recording");
        let export = ExportOptions {
            dir: dir.clone(),
            format: ExportFormat::Svg,
            record_max_frames: 10,
        };
        let mut app = test_app(export);
        let viewport = Rect::new(0, 0, 100, 40);

        toggle_recording(&mut app, viewport).unwrap();
        capture_recording_frame(&mut app, viewport).unwrap();
        assert_eq!(app.recording.as_ref().unwrap().frame_count, 1);

        app.panels[0].series[0].value = Some(42.0);
        app.panels[0].series[0]
            .points
            .push((chrono::Utc::now().timestamp() as f64, 42.0));
        capture_recording_frame(&mut app, viewport).unwrap();
        assert_eq!(app.recording.as_ref().unwrap().frame_count, 2);

        toggle_recording(&mut app, viewport).unwrap();

        let recording_dir = fs::read_dir(&dir).unwrap().next().unwrap().unwrap().path();
        let manifest = recording_dir.join("manifest.json");
        assert!(manifest.exists());
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(manifest).unwrap()).unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["format"], "svg");
        assert_eq!(json["changed_only"], true);
        assert_eq!(json["viewport"]["width"], viewport.width);
        assert_eq!(json["viewport"]["height"], viewport.height);
        assert!(json["started_at"].as_str().unwrap().contains('T'));
        assert!(json["completed_at"].as_str().unwrap().contains('T'));
        assert_eq!(json["frames"].as_array().unwrap().len(), 2);
        assert_eq!(json["frames"][1]["index"], 2);
        assert!(
            json["frames"][1]["captured_at"]
                .as_str()
                .unwrap()
                .contains('T')
        );
        assert!(json["frames"][1]["elapsed_ms"].as_u64().is_some());
        assert_eq!(json["frames"][1]["files"][0], "frame-000002.svg");
        fs::remove_dir_all(dir).unwrap();
    }
}
