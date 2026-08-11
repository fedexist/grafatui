use std::collections::BTreeSet;
use std::fmt;

use super::{AnnotationSnapshot, AnnotationTarget};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AnnotationTargetWarning {
    MissingPanel { title: String },
    AmbiguousPanel { title: String, matches: usize },
}

impl fmt::Display for AnnotationTargetWarning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPanel { title } => {
                write!(
                    formatter,
                    "target \"{title}\" matches no graph/timeseries panel"
                )
            }
            Self::AmbiguousPanel { title, matches } => write!(
                formatter,
                "target \"{title}\" matches {matches} graph/timeseries panels; applied to all"
            ),
        }
    }
}

pub(crate) fn target_warnings(
    snapshot: &AnnotationSnapshot,
    eligible_panel_titles: &[String],
) -> Vec<AnnotationTargetWarning> {
    let requested = snapshot
        .events()
        .iter()
        .filter_map(|event| match &event.target {
            AnnotationTarget::All => None,
            AnnotationTarget::PanelTitles(titles) => Some(titles.iter()),
        })
        .flatten()
        .cloned()
        .collect::<BTreeSet<_>>();

    requested
        .into_iter()
        .filter_map(|title| {
            let matches = eligible_panel_titles
                .iter()
                .filter(|candidate| candidate.as_str() == title.as_str())
                .count();
            match matches {
                0 => Some(AnnotationTargetWarning::MissingPanel { title }),
                1 => None,
                matches => Some(AnnotationTargetWarning::AmbiguousPanel { title, matches }),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::annotations::{
        AnnotationSnapshot, AnnotationTarget, AnnotationTargetWarning, target_warnings,
    };

    fn targeted_event(text: &str, titles: &[&str]) -> crate::annotations::AnnotationEvent {
        let mut event = crate::annotations::test_event_at(10.0, text);
        event.target = AnnotationTarget::PanelTitles(
            titles.iter().map(|title| (*title).to_string()).collect(),
        );
        event
    }

    #[test]
    fn warns_once_for_missing_and_ambiguous_titles() {
        let snapshot = AnnotationSnapshot::new(vec![
            targeted_event("first", &["CPU", "Missing"]),
            targeted_event("second", &["CPU", "Missing"]),
        ]);
        let panels = vec!["CPU".to_string(), "CPU".to_string(), "Memory".to_string()];

        assert_eq!(
            target_warnings(&snapshot, &panels),
            vec![
                AnnotationTargetWarning::AmbiguousPanel {
                    title: "CPU".to_string(),
                    matches: 2,
                },
                AnnotationTargetWarning::MissingPanel {
                    title: "Missing".to_string(),
                },
            ]
        );
    }

    #[test]
    fn all_targets_do_not_produce_warnings() {
        let snapshot =
            AnnotationSnapshot::new(vec![crate::annotations::test_event_at(10.0, "all")]);

        assert!(target_warnings(&snapshot, &["CPU".to_string()]).is_empty());
    }

    #[test]
    fn formats_target_warnings_for_the_footer() {
        assert_eq!(
            AnnotationTargetWarning::AmbiguousPanel {
                title: "CPU".to_string(),
                matches: 2,
            }
            .to_string(),
            "target \"CPU\" matches 2 graph/timeseries panels; applied to all"
        );
        assert_eq!(
            AnnotationTargetWarning::MissingPanel {
                title: "Missing".to_string(),
            }
            .to_string(),
            "target \"Missing\" matches no graph/timeseries panel"
        );
    }
}
