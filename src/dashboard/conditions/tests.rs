use super::*;
fn group(show: bool, all: bool, items: Vec<Condition>) -> Conditions {
    Conditions { show, all, items }
}
fn variable(operator: Operator, value: &str) -> Condition {
    Condition::Variable {
        name: "node".into(),
        operator,
        value: value.into(),
    }
}
#[test]
fn conditions_compose_known_results_and_ignore_pending() {
    for show in [true, false] {
        for all in [true, false] {
            assert!(
                group(
                    show,
                    all,
                    vec![Condition::Data(true), Condition::TimeRange("bad".into())]
                )
                .visible(|_| None, None, Duration::from_secs(300))
            );
        }
    }
    assert!(
        !group(
            true,
            true,
            vec![Condition::Data(true), Condition::TimeRange("5m".into())]
        )
        .visible(|_| None, Some(false), Duration::from_secs(300))
    );
    assert!(
        group(
            true,
            false,
            vec![Condition::Data(true), Condition::TimeRange("5m".into())]
        )
        .visible(|_| None, Some(false), Duration::from_secs(300))
    );
    assert!(group(false, true, vec![Condition::Data(true)]).visible(
        |_| None,
        Some(false),
        Duration::from_secs(300)
    ));
    assert!(!group(false, false, vec![Condition::Data(true)]).visible(
        |_| None,
        Some(true),
        Duration::from_secs(300)
    ));
}
#[test]
fn conditions_variable_membership_all_and_javascript_regex() {
    for (operator, pattern, want) in [
        (Operator::Equals, "node-2", true),
        (Operator::NotEquals, "node-2", false),
        (Operator::Matches, "(?i)^NODE-(?=2)2$", true),
        (Operator::NotMatches, "(?i)^NODE-(?=2)2$", false),
        (Operator::Matches, "[", true),
        (Operator::NotMatches, "[", true),
        (Operator::Matches, r"^(node)-2\1$", false),
    ] {
        let result = group(true, true, vec![variable(operator, pattern)]).visible(
            |_| {
                Some(ConditionValue {
                    values: vec!["node-1".into(), "node-2".into()],
                    all: false,
                })
            },
            None,
            Duration::ZERO,
        );
        assert_eq!(result, want, "{operator:?} {pattern}");
    }
    assert!(
        group(true, true, vec![variable(Operator::Equals, "all")]).visible(
            |_| Some(ConditionValue {
                values: vec!["$__all".into()],
                all: true
            }),
            None,
            Duration::ZERO
        )
    );
    assert!(
        !group(true, true, vec![variable(Operator::Matches, "^.$")]).visible(
            |_| Some(ConditionValue {
                values: vec!["😀".into()],
                all: false
            }),
            None,
            Duration::ZERO
        )
    );
    assert!(
        group(true, true, vec![variable(Operator::Matches, "^..$")]).visible(
            |_| Some(ConditionValue {
                values: vec!["😀".into()],
                all: false
            }),
            None,
            Duration::ZERO
        )
    );
    assert!(
        group(true, true, vec![variable(Operator::Matches, r"^(a)\1$")]).visible(
            |_| Some(ConditionValue {
                values: vec!["aa".into()],
                all: false
            }),
            None,
            Duration::ZERO
        )
    );
    assert!(
        !group(true, true, vec![variable(Operator::Matches, "(?i)A(?-i)")]).visible(
            |_| Some(ConditionValue {
                values: vec!["a".into()],
                all: false
            }),
            None,
            Duration::ZERO
        )
    );
}
#[test]
fn conditions_range_uses_upstream_interval_units_and_equality() {
    for (interval, secs, want) in [
        ("5m", 300, true),
        ("5m", 301, false),
        ("1M", 2_592_000, true),
        ("1M", 2_592_001, false),
        ("1y", 31_536_000, true),
        ("1.5h", 3601, false),
        ("250ms", 1000, true),
        ("bad", 1000, true),
        ("0s", 1, false),
    ] {
        assert_eq!(
            group(true, true, vec![Condition::TimeRange(interval.into())]).visible(
                |_| None,
                None,
                Duration::from_secs(secs)
            ),
            want,
            "{interval} {secs}"
        );
    }
}
