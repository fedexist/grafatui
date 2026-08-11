use std::path::PathBuf;

mod details;
mod jsonl;
mod model;
mod projection;

pub(crate) use details::format_cluster_detail_lines;
pub(crate) use jsonl::{JsonlFileSource, SourcePoll};
pub(crate) use model::{
    AnnotationEvent, AnnotationPanelContext, AnnotationSnapshot, AnnotationTarget,
};
pub(crate) use projection::{AnnotationCluster, cluster_events_by};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AnnotationSourceStatus {
    #[cfg(test)]
    Disabled,
    Loaded(usize),
    Warning(String),
}

#[derive(Debug)]
pub(crate) enum AnnotationState {
    Disabled,
    Active {
        source: Option<JsonlFileSource>,
        snapshot: AnnotationSnapshot,
        visible: bool,
        status: AnnotationSourceStatus,
    },
}

impl AnnotationState {
    pub(crate) fn from_path(path: Option<PathBuf>) -> Self {
        match path {
            Some(path) => Self::Active {
                source: Some(JsonlFileSource::new(path)),
                snapshot: AnnotationSnapshot::new(Vec::new()),
                visible: true,
                status: AnnotationSourceStatus::Loaded(0),
            },
            None => Self::Disabled,
        }
    }

    pub(crate) async fn refresh_if_changed(&mut self) {
        let poll = match self {
            Self::Active {
                source: Some(source),
                ..
            } => source.poll().await,
            Self::Disabled | Self::Active { source: None, .. } => return,
        };

        match (self, poll) {
            (
                Self::Active {
                    snapshot, status, ..
                },
                SourcePoll::Loaded(next_snapshot),
            ) => {
                *snapshot = next_snapshot;
                *status = AnnotationSourceStatus::Loaded(snapshot.len());
            }
            (
                Self::Active {
                    snapshot, status, ..
                },
                SourcePoll::Failed(error),
            ) => {
                *status = AnnotationSourceStatus::Warning(format!(
                    "{error}; using {} previous event(s)",
                    snapshot.len()
                ));
            }
            (_, SourcePoll::Unchanged) => {}
            (Self::Disabled, _) => {}
        }
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> Option<&AnnotationSnapshot> {
        match self {
            Self::Disabled => None,
            Self::Active { snapshot, .. } => Some(snapshot),
        }
    }

    #[cfg(test)]
    pub(crate) fn status(&self) -> AnnotationSourceStatus {
        match self {
            Self::Disabled => AnnotationSourceStatus::Disabled,
            Self::Active { status, .. } => status.clone(),
        }
    }

    pub(crate) fn warning(&self) -> Option<&str> {
        match self {
            Self::Active {
                status: AnnotationSourceStatus::Warning(warning),
                ..
            } => Some(warning),
            Self::Disabled | Self::Active { .. } => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn is_configured(&self) -> bool {
        matches!(self, Self::Active { .. })
    }

    #[cfg(test)]
    pub(crate) fn is_visible(&self) -> bool {
        matches!(self, Self::Active { visible: true, .. })
    }

    pub(crate) fn toggle_visibility(&mut self) {
        if let Self::Active { visible, .. } = self {
            *visible = !*visible;
        }
    }

    pub(crate) fn events_for_panel(
        &self,
        panel: AnnotationPanelContext<'_>,
        bounds: [f64; 2],
    ) -> Vec<&AnnotationEvent> {
        match self {
            Self::Active {
                snapshot,
                visible: true,
                ..
            } => projection::events_for_panel(snapshot, panel, bounds),
            Self::Disabled | Self::Active { .. } => Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_events_for_test(events: Vec<AnnotationEvent>) -> Self {
        let snapshot = AnnotationSnapshot::new(events);
        let event_count = snapshot.len();
        Self::Active {
            source: None,
            snapshot,
            visible: true,
            status: AnnotationSourceStatus::Loaded(event_count),
        }
    }

    #[cfg(test)]
    pub(crate) fn warning_for_test(message: &str) -> Self {
        Self::Active {
            source: None,
            snapshot: AnnotationSnapshot::new(Vec::new()),
            visible: true,
            status: AnnotationSourceStatus::Warning(message.to_string()),
        }
    }
}

#[cfg(test)]
pub(crate) fn test_event_at(timestamp_secs: f64, text: &str) -> AnnotationEvent {
    let millis = (timestamp_secs * 1_000.0).round() as i64;
    AnnotationEvent {
        time: chrono::DateTime::<chrono::Utc>::from_timestamp_millis(millis).unwrap(),
        text: text.to_string(),
        tags: Vec::new(),
        target: AnnotationTarget::All,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{AnnotationSourceStatus, AnnotationState};

    fn temp_path(name: &str) -> PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "grafatui-annotations-{name}-{}-{suffix}.jsonl",
            std::process::id()
        ))
    }

    #[tokio::test]
    async fn state_retains_last_valid_snapshot_and_clears_warning_on_recovery() {
        let path = temp_path("state-recovery");
        tokio::fs::write(
            &path,
            "{\"time\":\"2026-07-23T14:30:00Z\",\"text\":\"deploy\"}\n",
        )
        .await
        .unwrap();
        let mut state = AnnotationState::from_path(Some(path.clone()));

        state.refresh_if_changed().await;
        assert_eq!(state.snapshot().unwrap().len(), 1);
        assert!(state.warning().is_none());

        tokio::fs::write(&path, "{\"time\":").await.unwrap();
        state.refresh_if_changed().await;
        assert_eq!(state.snapshot().unwrap().len(), 1);
        assert!(state.warning().unwrap().contains("using 1 previous event"));

        tokio::fs::write(&path, "").await.unwrap();
        state.refresh_if_changed().await;
        assert_eq!(state.snapshot().unwrap().len(), 0);
        assert!(state.warning().is_none());

        tokio::fs::remove_file(path).await.unwrap();
    }

    #[tokio::test]
    async fn state_keeps_reloading_while_hidden_and_retains_snapshot_if_removed() {
        let path = temp_path("state-hidden");
        tokio::fs::write(
            &path,
            "{\"time\":\"2026-07-23T14:30:00Z\",\"text\":\"deploy\"}\n",
        )
        .await
        .unwrap();
        let mut state = AnnotationState::from_path(Some(path.clone()));
        state.toggle_visibility();

        state.refresh_if_changed().await;
        assert_eq!(state.snapshot().unwrap().len(), 1);
        assert!(!state.is_visible());

        tokio::fs::remove_file(&path).await.unwrap();
        state.refresh_if_changed().await;
        assert_eq!(state.snapshot().unwrap().len(), 1);
        assert!(state.warning().unwrap().contains("using 1 previous event"));
    }

    #[test]
    fn disabled_state_has_no_source_work_or_visibility() {
        let mut state = AnnotationState::from_path(None);
        assert!(!state.is_configured());
        assert!(!state.is_visible());
        assert!(state.snapshot().is_none());
        assert_eq!(state.status(), AnnotationSourceStatus::Disabled);
        state.toggle_visibility();
        assert!(!state.is_visible());
    }
}
