use super::{
    JsonObject, require_array_from, require_expected_kind, require_object_from, require_string_from,
};
use crate::dashboard::conditions::{Condition, Conditions, Operator};
use anyhow::{Result, anyhow, ensure};
pub(super) fn parse(spec: &JsonObject, path: &str) -> Result<Option<Conditions>> {
    if !spec.contains_key("conditionalRendering") {
        return Ok(None);
    }
    let path = format!("{path}.conditionalRendering");
    let group = require_object_from(spec, "conditionalRendering", &path)?;
    require_expected_kind(group, &path, "ConditionalRenderingGroup")?;
    let path = format!("{path}.spec");
    let spec = require_object_from(group, "spec", &path)?;
    let visibility = require_string_from(spec, "visibility", &format!("{path}.visibility"))?;
    ensure!(
        matches!(visibility, "show" | "hide"),
        "invalid Grafana V2 conditional visibility at {path}.visibility"
    );
    let condition = require_string_from(spec, "condition", &format!("{path}.condition"))?;
    ensure!(
        matches!(condition, "and" | "or"),
        "invalid Grafana V2 conditional composition at {path}.condition"
    );
    let mut items = Vec::new();
    for (index, item) in require_array_from(spec, "items", &format!("{path}.items"))?
        .iter()
        .enumerate()
    {
        let path = format!("{path}.items[{index}]");
        let item = item
            .as_object()
            .ok_or_else(|| anyhow!("invalid Grafana V2 condition at {path}: expected an object"))?;
        let kind = require_string_from(item, "kind", &format!("{path}.kind"))?;
        let kind_path = format!("{path}.kind");
        let path = format!("{path}.spec");
        let spec = require_object_from(item, "spec", &path)?;
        items.push(match kind {
            "ConditionalRenderingVariable" => {
                let name =
                    require_string_from(spec, "variable", &format!("{path}.variable"))?.to_string();
                let operator =
                    match require_string_from(spec, "operator", &format!("{path}.operator"))? {
                        "equals" => Operator::Equals,
                        "notEquals" => Operator::NotEquals,
                        "matches" => Operator::Matches,
                        "notMatches" => Operator::NotMatches,
                        _ => {
                            return Err(anyhow!(
                                "invalid Grafana V2 variable condition at {path}.operator"
                            ));
                        }
                    };
                let value =
                    require_string_from(spec, "value", &format!("{path}.value"))?.to_string();
                Condition::Variable {
                    name,
                    operator,
                    value,
                }
            }
            "ConditionalRenderingData" => {
                Condition::Data(spec.get("value").and_then(|v| v.as_bool()).ok_or_else(|| {
                    anyhow!("invalid Grafana V2 data condition at {path}.value: expected a boolean")
                })?)
            }
            "ConditionalRenderingTimeRangeSize" => Condition::TimeRange(
                require_string_from(spec, "value", &format!("{path}.value"))?.to_string(),
            ),
            _ => {
                return Err(anyhow!(
                    "unsupported Grafana V2 condition `{kind}` at {kind_path}"
                ));
            }
        });
    }
    Ok(Some(Conditions {
        show: visibility == "show",
        all: condition == "and",
        items,
    }))
}
