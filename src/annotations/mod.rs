mod jsonl;
mod model;
mod projection;

pub(crate) use jsonl::{AnnotationLoadError, parse_jsonl};
pub(crate) use model::{
    AnnotationEvent, AnnotationPanelContext, AnnotationSnapshot, AnnotationTarget,
};
pub(crate) use projection::{AnnotationCluster, cluster_events_by, events_for_panel};

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
