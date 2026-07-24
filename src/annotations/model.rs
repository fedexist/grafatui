use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AnnotationTarget {
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnnotationEvent {
    pub(crate) time: DateTime<Utc>,
    pub(crate) text: String,
    pub(crate) tags: Vec<String>,
    pub(crate) target: AnnotationTarget,
}

impl AnnotationEvent {
    pub(crate) fn timestamp_secs(&self) -> f64 {
        self.time.timestamp() as f64
            + f64::from(self.time.timestamp_subsec_nanos()) / 1_000_000_000.0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AnnotationSnapshot {
    events: Vec<AnnotationEvent>,
}

impl AnnotationSnapshot {
    pub(crate) fn new(mut events: Vec<AnnotationEvent>) -> Self {
        events.sort_by_key(|event| event.time);
        Self { events }
    }

    pub(crate) fn events(&self) -> &[AnnotationEvent] {
        &self.events
    }

    pub(crate) fn len(&self) -> usize {
        self.events.len()
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AnnotationPanelContext<'a> {
    pub(crate) title: &'a str,
}

impl AnnotationTarget {
    pub(crate) fn applies_to(&self, panel: AnnotationPanelContext<'_>) -> bool {
        let _ = panel.title;
        matches!(self, Self::All)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::{AnnotationEvent, AnnotationSnapshot, AnnotationTarget};

    fn event(time: &str, text: &str) -> AnnotationEvent {
        AnnotationEvent {
            time: DateTime::parse_from_rfc3339(time)
                .unwrap()
                .with_timezone(&Utc),
            text: text.to_string(),
            tags: Vec::new(),
            target: AnnotationTarget::All,
        }
    }

    #[test]
    fn snapshot_sorts_events_stably() {
        let first = event("2026-07-23T14:30:00Z", "first");
        let earlier = event("2026-07-23T14:00:00Z", "earlier");
        let second = event("2026-07-23T14:30:00Z", "second");

        let snapshot = AnnotationSnapshot::new(vec![first, earlier, second]);
        let texts: Vec<_> = snapshot
            .events()
            .iter()
            .map(|event| event.text.as_str())
            .collect();

        assert_eq!(texts, vec!["earlier", "first", "second"]);
    }

    #[test]
    fn timestamp_secs_preserves_fraction_beyond_milliseconds() {
        let event = event("2026-07-23T14:30:00.123456789Z", "deploy");
        let expected = event.time.timestamp() as f64 + 0.123_456_789;

        assert!((event.timestamp_secs() - expected).abs() < 1e-7);
    }
}
