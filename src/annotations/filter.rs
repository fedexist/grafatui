use std::collections::{BTreeMap, BTreeSet};

use super::{AnnotationEvent, AnnotationSnapshot};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TagFilter {
    selected: BTreeSet<String>,
}

impl TagFilter {
    #[allow(dead_code)]
    pub(crate) fn from_selected(tags: impl IntoIterator<Item = String>) -> Self {
        Self {
            selected: tags.into_iter().collect(),
        }
    }

    pub(crate) fn matches(&self, event: &AnnotationEvent) -> bool {
        self.selected.is_empty() || event.tags.iter().any(|tag| self.selected.contains(tag))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    #[allow(dead_code)]
    pub(crate) fn selected(&self) -> &BTreeSet<String> {
        &self.selected
    }

    pub(crate) fn summary(&self) -> String {
        self.selected.iter().cloned().collect::<Vec<_>>().join("|")
    }

    #[allow(dead_code)]
    pub(crate) fn toggle(&mut self, tag: &str) {
        if !self.selected.remove(tag) {
            self.selected.insert(tag.to_string());
        }
    }

    #[allow(dead_code)]
    pub(crate) fn clear(&mut self) {
        self.selected.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct TagCatalogueEntry {
    pub(crate) tag: String,
    pub(crate) count: usize,
}

#[allow(dead_code)]
pub(crate) fn tag_catalogue(
    snapshot: &AnnotationSnapshot,
    filter: &TagFilter,
) -> Vec<TagCatalogueEntry> {
    let mut counts = BTreeMap::<String, usize>::new();
    for event in snapshot.events() {
        let event_tags = event
            .tags
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for tag in event_tags {
            *counts.entry(tag.to_string()).or_default() += 1;
        }
    }
    for tag in filter.selected() {
        counts.entry(tag.clone()).or_default();
    }
    counts
        .into_iter()
        .map(|(tag, count)| TagCatalogueEntry { tag, count })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotations::{AnnotationEvent, AnnotationSnapshot};

    fn event(tags: &[&str]) -> AnnotationEvent {
        let mut event = crate::annotations::test_event_at(10.0, "event");
        event.tags = tags.iter().map(|tag| (*tag).to_string()).collect();
        event
    }

    #[test]
    fn empty_filter_matches_all_and_selected_tags_use_exact_or_semantics() {
        assert!(TagFilter::default().matches(&event(&[])));

        let filter = TagFilter::from_selected(["deploy".to_string(), "incident".to_string()]);
        assert!(filter.matches(&event(&["deploy", "production"])));
        assert!(filter.matches(&event(&["incident"])));
        assert!(!filter.matches(&event(&["Deploy"])));
        assert!(!filter.matches(&event(&["production"])));
        assert!(!filter.matches(&event(&[])));
    }

    #[test]
    fn catalogue_counts_unique_events_and_keeps_selected_missing_tags() {
        let snapshot = AnnotationSnapshot::new(vec![
            event(&["deploy", "deploy", "production"]),
            event(&["deploy"]),
        ]);
        let filter = TagFilter::from_selected(["missing".to_string()]);

        assert_eq!(
            tag_catalogue(&snapshot, &filter),
            vec![
                TagCatalogueEntry {
                    tag: "deploy".to_string(),
                    count: 2,
                },
                TagCatalogueEntry {
                    tag: "missing".to_string(),
                    count: 0,
                },
                TagCatalogueEntry {
                    tag: "production".to_string(),
                    count: 1,
                },
            ]
        );
    }
}
