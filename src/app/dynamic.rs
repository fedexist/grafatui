//! Materialize AutoGrid templates without tying instance identity to display order.
use super::data::expand_expr;
use crate::{
    app::{AppMode, AppState, PanelState},
    dashboard::{
        DashboardItemId, DashboardLayout, DashboardLayoutItem,
        autogrid::{AutoGridBehavior, AutoGridItem},
        variables::{VariableOption, VariableState},
    },
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct InstanceKey {
    template: usize,
    value: Option<String>,
    occurrence: usize,
}
#[derive(Debug, Clone)]
struct Instance {
    key: InstanceKey,
    scope: usize,
    binding: Option<(String, VariableOption)>,
    query_data: Option<bool>,
}
#[derive(Debug)]
pub(crate) struct DynamicDashboard {
    templates: Vec<PanelState>,
    layout: DashboardLayout,
    expanded_layout: DashboardLayout,
    behaviors: HashMap<usize, AutoGridBehavior>,
    instances: Vec<Instance>,
}
impl DynamicDashboard {
    pub(crate) fn mark_query_result(&mut self, index: usize, has_data: bool) {
        if let Some(instance) = self.instances.get_mut(index) {
            instance.query_data = Some(has_data);
        }
    }
    fn visible(
        &self,
        index: usize,
        state: &VariableState,
        vars: &HashMap<String, String>,
        range: std::time::Duration,
    ) -> bool {
        let instance = &self.instances[index];
        let Some(conditions) = self
            .behaviors
            .get(&instance.key.template)
            .and_then(|b| b.conditions.as_ref())
        else {
            return true;
        };
        conditions.visible(
            |name| {
                use crate::dashboard::conditions::ConditionValue;
                if let Some((bound, value)) = &instance.binding
                    && bound == name
                {
                    return Some(ConditionValue {
                        values: vec![value.value.clone()],
                        all: false,
                    });
                }
                let variable = state.lookup(instance.scope, name);
                if let Some(variable) = variable {
                    if state.scopes[0]
                        .variables
                        .iter()
                        .any(|root| std::ptr::eq(root, variable))
                        && let Some(value) = state.overrides.get(name)
                    {
                        return Some(ConditionValue {
                            values: vec![value.clone()],
                            all: false,
                        });
                    }
                    return Some(ConditionValue {
                        values: variable.values.clone(),
                        all: variable.all,
                    });
                }
                vars.get(name).map(|value| ConditionValue {
                    values: vec![value.clone()],
                    all: false,
                })
            },
            instance.query_data,
            range,
        )
    }
    fn eligible(
        &self,
        index: usize,
        state: &VariableState,
        vars: &HashMap<String, String>,
        range: std::time::Duration,
    ) -> bool {
        let instance = &self.instances[index];
        self.behaviors
            .get(&instance.key.template)
            .and_then(|b| b.conditions.as_ref())
            .is_some_and(|c| c.has_data_predicate())
            || self.visible(index, state, vars, range)
    }
    pub(crate) fn values_for(
        &self,
        index: usize,
        state: &VariableState,
        fallback: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let Some(instance) = self.instances.get(index) else {
            return fallback.clone();
        };
        let mut values = state.scope_values(instance.scope, fallback);
        if let Some((name, value)) = &instance.binding {
            values.insert(name.clone(), value.value.clone());
        }
        values
    }
}
fn sync_container_state(items: &mut [DashboardLayoutItem], runtime: &DashboardLayout) {
    for item in items {
        match item {
            DashboardLayoutItem::Row(row) => {
                if let Some(previous) = runtime.row(row.id) {
                    row.collapsed = previous.collapsed;
                }
                sync_container_state(&mut row.children, runtime);
            }
            DashboardLayoutItem::Tabs(group) => {
                if let Some(previous) = runtime.tabs(group.id) {
                    group.active = previous.active;
                }
                for tab in &mut group.tabs {
                    sync_container_state(&mut tab.children, runtime);
                }
            }
            _ => {}
        }
    }
}
impl AppState {
    pub(crate) fn active_variable_scopes(&self) -> HashSet<usize> {
        // Conditions hide instances, not their section variables. Use source templates
        // and current container state so an inactive tab cannot issue variable queries.
        let mut layout = self
            .dynamic
            .as_ref()
            .map_or_else(|| self.layout.clone(), |d| d.layout.clone());
        sync_container_state(&mut layout.items, &self.layout);
        let mut scopes = HashSet::from([0]);
        for panel in layout.visible_panel_indices() {
            let mut scope = self
                .variable_state
                .panel_scopes
                .get(panel)
                .copied()
                .unwrap_or(0);
            while scopes.insert(scope) {
                match self.variable_state.scopes.get(scope).and_then(|s| s.parent) {
                    Some(parent) => scope = parent,
                    None => break,
                }
            }
        }
        scopes
    }

    pub(crate) fn configure_dynamic(&mut self, behaviors: HashMap<usize, AutoGridBehavior>) {
        if behaviors.is_empty() {
            return;
        }
        self.dynamic = Some(DynamicDashboard {
            templates: self.panels.clone(),
            layout: self.layout.clone(),
            expanded_layout: self.layout.clone(),
            behaviors,
            instances: Vec::new(),
        });
        self.reconcile_dynamic();
    }
    pub(crate) fn reconcile_dynamic(&mut self) -> Vec<usize> {
        let Some(mut dynamic) = self.dynamic.take() else {
            return Vec::new();
        };
        // Container IDs remain static; copy their interactive state before rebuilding children.
        sync_container_state(&mut dynamic.layout.items, &self.layout);
        let old_active: HashSet<_> = dynamic
            .expanded_layout
            .visible_panel_indices()
            .into_iter()
            .filter_map(|i| dynamic.instances.get(i).map(|i| i.key.clone()))
            .collect();
        let old_selected = self.selected_item;
        let old_instances = std::mem::take(&mut dynamic.instances);
        let mut old: HashMap<_, _> = old_instances
            .into_iter()
            .zip(std::mem::take(&mut self.panels))
            .enumerate()
            .map(|(index, (instance, panel))| (instance.key, (index, panel, instance.query_data)))
            .collect();
        let mut panels = Vec::new();
        let mut remap = HashMap::new();
        let mut layout = dynamic.layout.clone();
        #[allow(clippy::too_many_arguments)]
        fn materialize(
            items: &mut [DashboardLayoutItem],
            dynamic: &mut DynamicDashboard,
            state: &VariableState,
            vars: &HashMap<String, String>,
            range: std::time::Duration,
            step: std::time::Duration,
            old: &mut HashMap<InstanceKey, (usize, PanelState, Option<bool>)>,
            panels: &mut Vec<PanelState>,
            remap: &mut HashMap<usize, usize>,
        ) {
            #[allow(clippy::too_many_arguments)]
            fn add(
                template: usize,
                binding: Option<(String, VariableOption)>,
                occurrence: usize,
                dynamic: &mut DynamicDashboard,
                state: &VariableState,
                vars: &HashMap<String, String>,
                range: std::time::Duration,
                step: std::time::Duration,
                old: &mut HashMap<InstanceKey, (usize, PanelState, Option<bool>)>,
                panels: &mut Vec<PanelState>,
                remap: &mut HashMap<usize, usize>,
            ) -> usize {
                let key = InstanceKey {
                    template,
                    value: binding.as_ref().map(|(_, v)| v.value.clone()),
                    occurrence,
                };
                let index = panels.len();
                let mut query_data = None;
                let mut panel = if let Some((old_index, panel, data)) = old.remove(&key) {
                    query_data = data;
                    remap.insert(old_index, index);
                    panel
                } else {
                    dynamic.templates[template].clone()
                };
                let scope = state.panel_scopes.get(template).copied().unwrap_or(0);
                let mut texts = state.scope_values(scope, vars);
                for name in texts.clone().keys() {
                    if let Some(v) = state.lookup(scope, name) {
                        if state.scopes[0]
                            .variables
                            .iter()
                            .any(|root| std::ptr::eq(root, v))
                            && state.overrides.contains_key(name)
                        {
                            continue;
                        }
                        texts.insert(
                            name.clone(),
                            if v.all {
                                "All".into()
                            } else {
                                v.texts.join(", ")
                            },
                        );
                    }
                }
                if let Some((name, value)) = &binding {
                    texts.insert(name.clone(), value.text.clone());
                }
                panel.title = expand_expr(&dynamic.templates[template].title, range, step, &texts);
                dynamic.instances.push(Instance {
                    query_data,
                    key,
                    scope,
                    binding,
                });
                panels.push(panel);
                index
            }
            for item in items {
                match item {
                    DashboardLayoutItem::Panel(index) => {
                        *index = add(
                            *index, None, 0, dynamic, state, vars, range, step, old, panels, remap,
                        );
                    }
                    DashboardLayoutItem::AutoGrid(group) => {
                        let templates = std::mem::take(&mut group.items);
                        for template in templates {
                            let repeat = dynamic
                                .behaviors
                                .get(&template.index)
                                .and_then(|b| b.repeat.clone());
                            let scope =
                                state.panel_scopes.get(template.index).copied().unwrap_or(0);
                            let bindings: Vec<_> = match repeat {
                                None => vec![None],
                                Some(name) => match state.lookup(scope, &name) {
                                    Some(v) if !v.repeatable => vec![None],
                                    _ => {
                                        let mut values = state
                                            .repeat_values(scope, &name)
                                            .unwrap_or_else(|| {
                                                vec![VariableOption {
                                                    value: String::new(),
                                                    text: String::new(),
                                                }]
                                            });
                                        if values.is_empty() {
                                            values.push(VariableOption {
                                                value: String::new(),
                                                text: if state
                                                    .lookup(scope, &name)
                                                    .is_some_and(|v| v.all)
                                                {
                                                    "All".into()
                                                } else {
                                                    "None".into()
                                                },
                                            });
                                        }
                                        values
                                            .into_iter()
                                            .map(|v| Some((name.clone(), v)))
                                            .collect()
                                    }
                                },
                            };
                            let mut occurrences = HashMap::new();
                            for binding in bindings {
                                let value = binding.as_ref().map(|(_, v)| v.value.clone());
                                let occurrence = occurrences.entry(value).or_insert(0);
                                let index = add(
                                    template.index,
                                    binding,
                                    *occurrence,
                                    dynamic,
                                    state,
                                    vars,
                                    range,
                                    step,
                                    old,
                                    panels,
                                    remap,
                                );
                                *occurrence += 1;
                                group.items.push(AutoGridItem {
                                    index,
                                    fit_content: template.fit_content,
                                });
                            }
                        }
                    }
                    DashboardLayoutItem::Row(row) => materialize(
                        &mut row.children,
                        dynamic,
                        state,
                        vars,
                        range,
                        step,
                        old,
                        panels,
                        remap,
                    ),
                    DashboardLayoutItem::Tabs(group) => {
                        for tab in &mut group.tabs {
                            materialize(
                                &mut tab.children,
                                dynamic,
                                state,
                                vars,
                                range,
                                step,
                                old,
                                panels,
                                remap,
                            )
                        }
                    }
                }
            }
        }
        materialize(
            &mut layout.items,
            &mut dynamic,
            &self.variable_state,
            &self.vars,
            self.range,
            self.step,
            &mut old,
            &mut panels,
            &mut remap,
        );
        let map_item = |item| match item {
            DashboardItemId::Panel(index) => remap.get(&index).copied().map(DashboardItemId::Panel),
            other => Some(other),
        };
        self.selected_item = old_selected
            .and_then(map_item)
            .filter(|id| layout.visible_items().iter().any(|i| i.id == *id))
            .or_else(|| layout.first_visible());
        self.search_results = std::mem::take(&mut self.search_results)
            .into_iter()
            .filter_map(map_item)
            .collect();
        self.panel_body_scroll = std::mem::take(&mut self.panel_body_scroll)
            .into_iter()
            .filter_map(|(old, state)| remap.get(&old).map(|&new| (new, state)))
            .collect();
        self.panels = panels;
        let newly_active = layout
            .visible_panel_indices()
            .into_iter()
            .filter(|&i| !old_active.contains(&dynamic.instances[i].key))
            .collect();
        dynamic.expanded_layout = layout.clone();
        self.layout = layout;
        if self.selected_panel_index().is_none() {
            self.mode = match self.mode {
                AppMode::Fullscreen => AppMode::Normal,
                AppMode::FullscreenInspect => AppMode::Inspect,
                mode => mode,
            };
        }
        self.dynamic = Some(dynamic);
        self.evaluate_dynamic_visibility();
        newly_active
    }
    pub(crate) fn query_eligible_indices(&self) -> Vec<usize> {
        match &self.dynamic {
            None => self.visible_panel_indices(),
            Some(dynamic) => dynamic
                .expanded_layout
                .visible_panel_indices()
                .into_iter()
                .filter(|&i| dynamic.eligible(i, &self.variable_state, &self.vars, self.range))
                .collect(),
        }
    }
    pub(crate) fn evaluate_dynamic_visibility(&mut self) {
        let Some(dynamic) = &self.dynamic else { return };
        let visible: Vec<_> = (0..self.panels.len())
            .map(|i| dynamic.visible(i, &self.variable_state, &self.vars, self.range))
            .collect();
        self.layout = dynamic.expanded_layout.clone();
        fn filter(items: &mut [DashboardLayoutItem], visible: &[bool]) {
            for item in items {
                match item {
                    DashboardLayoutItem::AutoGrid(grid) => {
                        grid.items.retain(|item| visible[item.index])
                    }
                    DashboardLayoutItem::Row(row) => filter(&mut row.children, visible),
                    DashboardLayoutItem::Tabs(group) => {
                        for tab in &mut group.tabs {
                            filter(&mut tab.children, visible)
                        }
                    }
                    DashboardLayoutItem::Panel(_) => {}
                }
            }
        }
        filter(&mut self.layout.items, &visible);
        if !self
            .layout
            .visible_items()
            .iter()
            .any(|i| Some(i.id) == self.selected_item)
        {
            self.selected_item = self.layout.first_visible();
        }
        if self.selected_panel_index().is_none() {
            self.mode = match self.mode {
                AppMode::Fullscreen => AppMode::Normal,
                AppMode::FullscreenInspect => AppMode::Inspect,
                mode => mode,
            };
        }
        if self.mode == AppMode::Search {
            super::input::update_search_results(self);
        }
    }
}
#[cfg(test)]
mod tests;
