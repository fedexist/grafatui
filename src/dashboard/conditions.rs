use std::time::Duration;
#[derive(Debug, Clone, Copy)]
pub(crate) enum Operator {
    Equals,
    NotEquals,
    Matches,
    NotMatches,
}
#[derive(Debug, Clone)]
pub(crate) enum Condition {
    Variable {
        name: String,
        operator: Operator,
        value: String,
    },
    Data(bool),
    TimeRange(String),
}
#[derive(Debug, Clone)]
pub(crate) struct ConditionValue {
    pub(crate) values: Vec<String>,
    pub(crate) all: bool,
}
#[derive(Debug, Clone)]
pub(crate) struct Conditions {
    pub(crate) show: bool,
    pub(crate) all: bool,
    pub(crate) items: Vec<Condition>,
}
impl Conditions {
    pub(crate) fn has_data_predicate(&self) -> bool {
        self.items.iter().any(|c| matches!(c, Condition::Data(_)))
    }
    pub(crate) fn visible(
        &self,
        variable: impl Fn(&str) -> Option<ConditionValue>,
        data: Option<bool>,
        range: Duration,
    ) -> bool {
        let known: Vec<_> = self
            .items
            .iter()
            .filter_map(|condition| match condition {
                Condition::Data(wanted) => data.map(|has_data| has_data == *wanted),
                Condition::TimeRange(value) => {
                    interval_seconds(value).map(|threshold| range.as_secs_f64() <= threshold)
                }
                Condition::Variable {
                    name,
                    operator,
                    value,
                } => {
                    if name.is_empty() {
                        return None;
                    }
                    let current = variable(name)?;
                    let hit = match operator {
                        Operator::Equals | Operator::NotEquals => {
                            current.values.contains(value)
                                || (current.all && value.eq_ignore_ascii_case("All"))
                        }
                        Operator::Matches | Operator::NotMatches => {
                            // Grafana treats invalid expressions as true, including notMatches.
                            let Ok(regex) = value_regex(value) else {
                                return Some(true);
                            };
                            current.values.iter().any(|value| {
                                regex
                                    .find_from_ucs2(&value.encode_utf16().collect::<Vec<_>>(), 0)
                                    .next()
                                    .is_some()
                            })
                        }
                    };
                    Some(
                        if matches!(operator, Operator::NotEquals | Operator::NotMatches) {
                            !hit
                        } else {
                            hit
                        },
                    )
                }
            })
            .collect();
        if known.is_empty() {
            return true;
        }
        let result = if self.all {
            known.iter().all(|r| *r)
        } else {
            known.iter().any(|r| *r)
        };
        if self.show { result } else { !result }
    }
}
fn value_regex(pattern: &str) -> Result<regress::Regex, regress::Error> {
    use std::sync::OnceLock;
    static INLINE: OnceLock<regex::Regex> = OnceLock::new();
    let inline = INLINE.get_or_init(|| regex::Regex::new(r"\(\?([ims-]+)\)").unwrap());
    let mut flags = std::collections::HashSet::new();
    let cleaned = inline.replace_all(pattern, |captures: &regex::Captures| {
        let group = captures[1].chars().collect::<Vec<_>>();
        let clear = group.first() == Some(&'-');
        for (i, flag) in group.iter().copied().enumerate() {
            if clear || i > 0 && group[i - 1] == '-' {
                flags.remove(&flag);
            } else if flag != '-' {
                flags.insert(flag);
            }
        }
        String::new()
    });
    let flags = ['i', 'm', 's']
        .into_iter()
        .filter(|f| flags.contains(f))
        .collect::<String>();
    regress::Regex::from_unicode(cleaned.encode_utf16().map(u32::from), flags.as_str())
}
fn interval_seconds(value: &str) -> Option<f64> {
    use std::sync::OnceLock;
    static INTERVAL: OnceLock<regex::Regex> = OnceLock::new();
    // Match the condition validator, then rangeUtil's parseInt count and fixed month/year units.
    let captures = INTERVAL
        .get_or_init(|| regex::Regex::new(r"^(\d+(?:\.\d+)?)([Mwdhmsy])$").unwrap())
        .captures(value)?;
    let count = captures[1].split('.').next()?.parse::<f64>().ok()?;
    let unit = match &captures[2] {
        "y" => 31_536_000.,
        "M" => 2_592_000.,
        "w" => 604_800.,
        "d" => 86_400.,
        "h" => 3600.,
        "m" => 60.,
        "s" => 1.,
        _ => return None,
    };
    Some(count * unit)
}
#[cfg(test)]
mod tests;
