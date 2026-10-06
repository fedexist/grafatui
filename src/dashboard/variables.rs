//! Retained Grafana selections and lexical variable scopes.
use crate::grafana::TemplateQueryVar;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VariableOption {
    pub(crate) value: String,
    pub(crate) text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Variable {
    pub(crate) name: String,
    pub(crate) values: Vec<String>,
    pub(crate) texts: Vec<String>,
    pub(crate) options: Vec<VariableOption>,
    pub(crate) all: bool,
    pub(crate) all_value: Option<String>,
    pub(crate) repeatable: bool,
    pub(crate) query: Option<TemplateQueryVar>,
    pub(crate) last_error: Option<String>,
}
impl Variable {
    pub(crate) fn repeat_values(&self) -> Vec<VariableOption> {
        if self.all {
            return self
                .options
                .iter()
                .filter(|o| o.value != "$__all")
                .cloned()
                .collect();
        }
        self.values
            .iter()
            .enumerate()
            .map(|(i, value)| VariableOption {
                value: value.clone(),
                text: self.texts.get(i).cloned().unwrap_or_else(|| value.clone()),
            })
            .collect()
    }
    pub(crate) fn query_value(&self) -> String {
        if self.all
            && let Some(value) = &self.all_value
            && !value.is_empty()
        {
            return value.clone();
        }
        let values = self.repeat_values();
        match values.as_slice() {
            [] if self.all => ".*".to_string(),
            [] => String::new(),
            [one] => one.value.clone(),
            _ => format!(
                "({})",
                values
                    .iter()
                    .map(|v| regex::escape(&v.value))
                    .collect::<Vec<_>>()
                    .join("|")
            ),
        }
    }
    pub(crate) fn update_options(&mut self, options: Vec<VariableOption>) {
        self.options = options;
        if !self.all {
            let selected: Vec<_> = self
                .values
                .iter()
                .filter_map(|value| self.options.iter().find(|o| &o.value == value).cloned())
                .collect();
            let selected = if selected.is_empty() {
                self.options.first().cloned().into_iter().collect()
            } else {
                selected
            };
            self.values = selected.iter().map(|o| o.value.clone()).collect();
            self.texts = selected.into_iter().map(|o| o.text).collect();
        }
        self.last_error = None;
    }
}
#[derive(Debug, Clone, Default)]
pub(crate) struct VariableScope {
    pub(crate) parent: Option<usize>,
    pub(crate) variables: Vec<Variable>,
}
#[derive(Debug, Clone)]
pub(crate) struct VariableState {
    pub(crate) scopes: Vec<VariableScope>,
    pub(crate) panel_scopes: Vec<usize>,
    pub(crate) overrides: HashMap<String, String>,
}
impl Default for VariableState {
    fn default() -> Self {
        Self {
            scopes: vec![VariableScope::default()],
            panel_scopes: Vec::new(),
            overrides: HashMap::new(),
        }
    }
}
impl VariableState {
    pub(crate) fn has_variables(&self) -> bool {
        self.scopes.iter().any(|s| !s.variables.is_empty())
    }
    pub(crate) fn lookup(&self, mut scope: usize, name: &str) -> Option<&Variable> {
        loop {
            let current = self.scopes.get(scope)?;
            if let Some(v) = current.variables.iter().rev().find(|v| v.name == name) {
                return Some(v);
            }
            scope = current.parent?;
        }
    }
    pub(crate) fn scope_values(
        &self,
        scope: usize,
        fallback: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut values = match self.scopes.get(scope).and_then(|s| s.parent) {
            Some(parent) => self.scope_values(parent, fallback),
            None => fallback.clone(),
        };
        if let Some(s) = self.scopes.get(scope) {
            for v in &s.variables {
                values.insert(
                    v.name.clone(),
                    if scope == 0 {
                        self.overrides
                            .get(&v.name)
                            .cloned()
                            .unwrap_or_else(|| v.query_value())
                    } else {
                        v.query_value()
                    },
                );
            }
        }
        values
    }
    pub(crate) fn repeat_values(&self, scope: usize, name: &str) -> Option<Vec<VariableOption>> {
        let v = self.lookup(scope, name)?;
        if !v.repeatable {
            return None;
        }
        // CLI/config overrides select the root variable, without rewriting section bindings.
        if self.scopes[0]
            .variables
            .iter()
            .any(|root| std::ptr::eq(root, v))
            && let Some(value) = self.overrides.get(name)
        {
            return Some(vec![VariableOption {
                value: value.clone(),
                text: value.clone(),
            }]);
        }
        Some(v.repeat_values())
    }
}
