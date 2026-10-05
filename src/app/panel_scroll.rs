//! Persistent panel body state, independent of dashboard scrolling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PanelBodyIdentity {
    Table,
    Error(String),
}

#[derive(Debug, Clone)]
pub(crate) struct PanelBodyScroll {
    pub(crate) identity: PanelBodyIdentity,
    pub(crate) offset: u64,
}

use super::AppState;
use crate::ui;
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy)]
pub(crate) enum BodyScrollAction {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

/// Refresh visible identities and bounds without changing focus or document scroll.
pub(crate) fn reconcile_panel_body_scroll(viewport: Rect, app: &mut AppState) {
    app.panel_body_scroll
        .retain(|index, _| *index < app.panels.len());
    for body in ui::panel_body_viewports(viewport, app) {
        let panel = &app.panels[body.index];
        let state = app
            .panel_body_scroll
            .entry(body.index)
            .or_insert_with(|| PanelBodyScroll {
                identity: panel
                    .last_error
                    .clone()
                    .map_or(PanelBodyIdentity::Table, PanelBodyIdentity::Error),
                offset: 0,
            });
        let same = match (&state.identity, &panel.last_error) {
            (PanelBodyIdentity::Table, None) => true,
            (PanelBodyIdentity::Error(previous), Some(current)) => previous == current,
            _ => false,
        };
        if !same {
            state.identity = panel
                .last_error
                .clone()
                .map_or(PanelBodyIdentity::Table, PanelBodyIdentity::Error);
            state.offset = 0;
        }
        if body.metrics.capacity > 0 {
            state.offset = state.offset.min(body.metrics.max_local_offset());
        }
    }
}

/// True means an eligible, nonempty body owns this input, including at its boundary.
pub(crate) fn scroll_selected_body(
    viewport: Rect,
    app: &mut AppState,
    action: BodyScrollAction,
) -> bool {
    reconcile_panel_body_scroll(viewport, app);
    let Some(body) = ui::panel_body_viewports(viewport, app)
        .into_iter()
        .find(|b| Some(b.index) == app.selected_panel_index() && b.metrics.capacity > 0)
    else {
        return false;
    };
    let state = app.panel_body_scroll.get_mut(&body.index).unwrap();
    let maximum = body.metrics.max_local_offset();
    let page = u64::from(body.metrics.capacity);
    state.offset = match action {
        BodyScrollAction::Up => state.offset.saturating_sub(1),
        BodyScrollAction::Down => state.offset.saturating_add(1).min(maximum),
        BodyScrollAction::PageUp => state.offset.saturating_sub(page),
        BodyScrollAction::PageDown => state.offset.saturating_add(page).min(maximum),
        BodyScrollAction::Home => 0,
        BodyScrollAction::End => maximum,
    };
    true
}
