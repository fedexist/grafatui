use super::{JsonObject, require_array_from, require_object_from, require_string_from};
use crate::dashboard::variables::{Variable, VariableOption};
use anyhow::{Result, anyhow, ensure};
use serde_json::Value;

fn strings(value: &Value, path: &str) -> Result<Vec<String>> {
    match value {
        Value::String(v) => Ok(vec![v.clone()]),
        Value::Array(values) => values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                v.as_str().map(str::to_string).ok_or_else(|| {
                    anyhow!("invalid Grafana V2 variable at {path}[{i}]: expected a string")
                })
            })
            .collect(),
        _ => Err(anyhow!(
            "invalid Grafana V2 variable at {path}: expected a string or string array"
        )),
    }
}
pub(super) fn retain(
    spec: &JsonObject,
    kind: &str,
    path: &str,
    query: Option<crate::grafana::TemplateQueryVar>,
) -> Result<Variable> {
    let name = require_string_from(spec, "name", &format!("{path}.name"))?.to_string();
    let (values, texts) = match spec.get("current") {
        None => (vec![], vec![]),
        Some(Value::String(value)) if kind == "SwitchVariable" => {
            (vec![value.clone()], vec![value.clone()])
        }
        Some(_) => {
            let current = require_object_from(spec, "current", &format!("{path}.current"))?;
            let values = current
                .get("value")
                .map(|v| strings(v, &format!("{path}.current.value")))
                .transpose()?
                .unwrap_or_default();
            let texts = current
                .get("text")
                .map(|v| strings(v, &format!("{path}.current.text")))
                .transpose()?
                .unwrap_or_else(|| values.clone());
            (values, texts)
        }
    };
    for field in ["multi", "includeAll"] {
        if let Some(v) = spec.get(field) {
            ensure!(
                v.is_boolean(),
                "invalid Grafana V2 variable at {path}.{field}: expected a boolean"
            );
        }
    }
    let all = values.iter().any(|v| v == "$__all");
    let mut options = Vec::new();
    if spec.contains_key("options") {
        for (i, option) in require_array_from(spec, "options", &format!("{path}.options"))?
            .iter()
            .enumerate()
        {
            let opath = format!("{path}.options[{i}]");
            let option = option.as_object().ok_or_else(|| {
                anyhow!("invalid Grafana V2 variable at {opath}: expected an object")
            })?;
            let vs = strings(
                option
                    .get("value")
                    .ok_or_else(|| anyhow!("missing variable value at {opath}.value"))?,
                &format!("{opath}.value"),
            )?;
            let ts = strings(
                option
                    .get("text")
                    .ok_or_else(|| anyhow!("missing variable text at {opath}.text"))?,
                &format!("{opath}.text"),
            )?;
            for (n, value) in vs.into_iter().enumerate() {
                options.push(VariableOption {
                    text: ts.get(n).cloned().unwrap_or_else(|| value.clone()),
                    value,
                });
            }
        }
    }
    if kind == "CustomVariable"
        && options.is_empty()
        && let Some(query) = spec.get("query")
    {
        let query = query.as_str().ok_or_else(|| {
            anyhow!("invalid Grafana V2 variable at {path}.query: expected a string")
        })?;
        options = custom_options(query);
    }
    let all_value = spec
        .get("allValue")
        .map(|v| {
            v.as_str().map(str::to_string).ok_or_else(|| {
                anyhow!("invalid Grafana V2 variable at {path}.allValue: expected a string")
            })
        })
        .transpose()?;
    Ok(Variable {
        name,
        values,
        texts,
        options,
        all,
        all_value,
        repeatable: matches!(
            kind,
            "QueryVariable"
                | "CustomVariable"
                | "DatasourceVariable"
                | "IntervalVariable"
                | "GroupByVariable"
        ),
        query,
        last_error: None,
    })
}
fn custom_options(query: &str) -> Vec<VariableOption> {
    let mut pieces = Vec::new();
    let mut part = String::new();
    let mut escaped = false;
    for ch in query.chars() {
        if escaped {
            part.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == ',' {
            pieces.push(std::mem::take(&mut part));
        } else {
            part.push(ch);
        }
    }
    if escaped {
        part.push('\\');
    }
    pieces.push(part);
    pieces
        .into_iter()
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            let (text, value) = p
                .split_once(" : ")
                .map(|(t, v)| (t.trim(), v.trim()))
                .unwrap_or((p.trim(), p.trim()));
            VariableOption {
                value: value.to_string(),
                text: text.to_string(),
            }
        })
        .collect()
}
