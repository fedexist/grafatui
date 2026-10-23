/*
 * Copyright 2026 Federico D'Ambrosio
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) struct DisplayNameTemplate {
    parts: Vec<Part>,
}

#[derive(Debug, Clone)]
enum Part {
    Literal(String),
    DefaultName,
    FieldName,
    Labels,
    LabelValues,
    Label(String),
}

impl DisplayNameTemplate {
    pub(crate) fn parse(input: &str) -> Result<Self, String> {
        const OPEN: &str = concat!("$", "{");
        let mut parts = Vec::new();
        let mut remaining = input;

        while let Some(start) = remaining.find(OPEN) {
            if start > 0 {
                parts.push(Part::Literal(remaining[..start].to_string()));
            }
            let expression = &remaining[start + OPEN.len()..];
            let end = expression
                .find('}')
                .ok_or_else(|| "unclosed display-name expression".to_string())?;
            let token = &expression[..end];
            let part = match token {
                "__field.displayName" => Part::DefaultName,
                "__field.name" => Part::FieldName,
                "__field.labels" => Part::Labels,
                "__field.labels.__values" => Part::LabelValues,
                _ => {
                    let key = token
                        .strip_prefix("__field.labels.")
                        .filter(|key| valid_label_key(key))
                        .ok_or_else(|| format!("unsupported display-name expression: {token}"))?;
                    Part::Label(key.to_string())
                }
            };
            parts.push(part);
            remaining = &expression[end + 1..];
        }
        if !remaining.is_empty() {
            parts.push(Part::Literal(remaining.to_string()));
        }
        Ok(Self { parts })
    }

    pub(crate) fn render(
        &self,
        default_name: &str,
        field_name: &str,
        labels: &HashMap<String, String>,
    ) -> String {
        let mut sorted_labels: Vec<_> = labels.iter().collect();
        sorted_labels.sort_by_key(|(key, _)| *key);
        let mut output = String::new();
        for part in &self.parts {
            match part {
                Part::Literal(value) => output.push_str(value),
                Part::DefaultName => output.push_str(default_name),
                Part::FieldName => output.push_str(field_name),
                Part::Labels => {
                    for (index, (key, value)) in sorted_labels.iter().enumerate() {
                        if index > 0 {
                            output.push_str(", ");
                        }
                        output.push_str(key);
                        output.push_str("=\"");
                        output.push_str(value);
                        output.push('"');
                    }
                }
                Part::LabelValues => {
                    for (index, (_, value)) in sorted_labels.iter().enumerate() {
                        if index > 0 {
                            output.push_str(", ");
                        }
                        output.push_str(value);
                    }
                }
                Part::Label(key) => output.push_str(labels.get(key).map_or("", String::as_str)),
            }
        }
        if output.is_empty() {
            default_name.to_string()
        } else {
            output
        }
    }
}

fn valid_label_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::DisplayNameTemplate;
    use std::collections::HashMap;

    fn labels() -> HashMap<String, String> {
        HashMap::from([
            ("b".to_string(), "2".to_string()),
            ("a".to_string(), "1".to_string()),
        ])
    }

    #[test]
    fn parses_all_documented_expressions() {
        for input in [
            "${__field.displayName}",
            "${__field.name}",
            "${__field.labels}",
            "${__field.labels.a}",
            "${__field.labels.__values}",
        ] {
            assert!(DisplayNameTemplate::parse(input).is_ok(), "{input}");
        }
    }

    #[test]
    fn renders_mixed_and_repeated_expressions() {
        let template = DisplayNameTemplate::parse(
            "${__field.displayName} / ${__field.name} / ${__field.labels} / ${__field.labels.__values}",
        )
        .unwrap();
        let rendered_all = template.render("alias", "Value", &labels());
        assert_eq!(rendered_all, "alias / Value / a=\"1\", b=\"2\" / 1, 2");

        let repeated =
            DisplayNameTemplate::parse("${__field.labels.a}-${__field.labels.a}").unwrap();
        assert_eq!(repeated.render("alias", "Value", &labels()), "1-1");
    }

    #[test]
    fn sorts_label_pairs_and_values() {
        let template =
            DisplayNameTemplate::parse("${__field.labels} | ${__field.labels.__values}").unwrap();
        assert_eq!(
            template.render("alias", "Value", &labels()),
            "a=\"1\", b=\"2\" | 1, 2"
        );
    }

    #[test]
    fn falls_back_when_result_empty() {
        let missing = DisplayNameTemplate::parse("${__field.labels.missing}").unwrap();
        assert_eq!(missing.render("alias", "Value", &labels()), "alias");

        let empty = DisplayNameTemplate::parse("${__field.labels}").unwrap();
        assert_eq!(empty.render("alias", "Value", &HashMap::new()), "alias");

        let literal_spaces = DisplayNameTemplate::parse("  ").unwrap();
        assert_eq!(
            literal_spaces.render("alias", "Value", &HashMap::new()),
            "  "
        );
    }

    #[test]
    fn rejects_unknown_or_unclosed_expression() {
        for input in [
            "${__field.unknown}",
            "prefix ${__field.labels.a",
            "${__field.labels.}",
            "${__field.labels.__values.extra}",
        ] {
            assert!(DisplayNameTemplate::parse(input).is_err(), "{input}");
        }
    }

    #[test]
    fn preserves_special_label_values() {
        let labels = HashMap::from([("a".to_string(), "ü\"\u{5c}${__field.name}".to_string())]);
        let template = DisplayNameTemplate::parse("${__field.labels.a}").unwrap();
        assert_eq!(
            template.render("alias", "Value", &labels),
            "ü\"\u{5c}${__field.name}"
        );
    }
}
