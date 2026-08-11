use std::collections::BTreeMap;

use super::{AnnotationEvent, AnnotationPanelContext, AnnotationSnapshot};

#[derive(Debug)]
pub(crate) struct AnnotationCluster<'a> {
    pub(crate) coordinate: u32,
    pub(crate) events: Vec<&'a AnnotationEvent>,
}

pub(crate) fn events_for_panel<'a>(
    snapshot: &'a AnnotationSnapshot,
    panel: AnnotationPanelContext<'_>,
    bounds: [f64; 2],
) -> Vec<&'a AnnotationEvent> {
    snapshot
        .events()
        .iter()
        .filter(|event| {
            let timestamp = event.timestamp_secs();
            timestamp >= bounds[0] && timestamp <= bounds[1] && event.target.applies_to(panel)
        })
        .collect()
}

pub(crate) fn cluster_events_by<'a>(
    events: impl IntoIterator<Item = &'a AnnotationEvent>,
    mut project: impl FnMut(f64) -> Option<u32>,
) -> Vec<AnnotationCluster<'a>> {
    let mut grouped: BTreeMap<u32, Vec<&'a AnnotationEvent>> = BTreeMap::new();
    for event in events {
        if let Some(coordinate) = project(event.timestamp_secs()) {
            grouped.entry(coordinate).or_default().push(event);
        }
    }
    grouped
        .into_iter()
        .map(|(coordinate, events)| AnnotationCluster { coordinate, events })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{cluster_events_by, events_for_panel};
    use crate::annotations::{AnnotationEvent, AnnotationPanelContext, AnnotationSnapshot};

    fn event(timestamp: f64, text: &str) -> AnnotationEvent {
        crate::annotations::test_event_at(timestamp, text)
    }

    #[test]
    fn routes_all_targets_and_filters_inclusive_time_bounds() {
        let snapshot = AnnotationSnapshot::new(vec![
            event(0.0, "start"),
            event(50.0, "middle"),
            event(100.0, "end"),
            event(101.0, "outside"),
        ]);

        let events = events_for_panel(
            &snapshot,
            AnnotationPanelContext { title: "CPU" },
            [0.0, 100.0],
        );

        assert_eq!(
            events
                .iter()
                .map(|event| event.text.as_str())
                .collect::<Vec<_>>(),
            vec!["start", "middle", "end"]
        );
    }

    #[test]
    fn clusters_events_by_projected_coordinate_in_order() {
        let snapshot = AnnotationSnapshot::new(vec![
            event(10.0, "one"),
            event(11.0, "two"),
            event(90.0, "three"),
        ]);
        let events = events_for_panel(
            &snapshot,
            AnnotationPanelContext { title: "CPU" },
            [0.0, 100.0],
        );

        let clusters = cluster_events_by(events, |timestamp| {
            Some(if timestamp < 50.0 { 4 } else { 9 })
        });

        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0].coordinate, 4);
        assert_eq!(
            clusters[0]
                .events
                .iter()
                .map(|event| event.text.as_str())
                .collect::<Vec<_>>(),
            vec!["one", "two"]
        );
    }
}
