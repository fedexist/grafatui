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
use std::collections::HashMap;

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
}
#[derive(Debug)]
pub(crate) struct DynamicDashboard {
    templates: Vec<PanelState>,
    layout: DashboardLayout,
    behaviors: HashMap<usize, AutoGridBehavior>,
    instances: Vec<Instance>,
}
impl DynamicDashboard {
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
impl AppState {
    pub(crate) fn configure_dynamic(&mut self, behaviors: HashMap<usize, AutoGridBehavior>) {
        if behaviors.is_empty() {
            return;
        }
        self.dynamic = Some(DynamicDashboard {
            templates: self.panels.clone(),
            layout: self.layout.clone(),
            behaviors,
            instances: Vec::new(),
        });
        self.reconcile_dynamic();
    }
    pub(crate) fn reconcile_dynamic(&mut self) {
        let Some(mut dynamic) = self.dynamic.take() else {
            return;
        };
        // Container IDs remain static; copy their interactive state before rebuilding children.
        fn sync(items: &mut [DashboardLayoutItem], runtime: &DashboardLayout) {
            for item in items {
                match item {
                    DashboardLayoutItem::Row(row) => {
                        if let Some(previous) = runtime.row(row.id) {
                            row.collapsed = previous.collapsed;
                        }
                        sync(&mut row.children, runtime);
                    }
                    DashboardLayoutItem::Tabs(group) => {
                        if let Some(previous) = runtime.tabs(group.id) {
                            group.active = previous.active;
                        }
                        for tab in &mut group.tabs {
                            sync(&mut tab.children, runtime);
                        }
                    }
                    _ => {}
                }
            }
        }
        sync(&mut dynamic.layout.items, &self.layout);
        let old_selected = self.selected_item;
        let old_instances = std::mem::take(&mut dynamic.instances);
        let mut old: HashMap<_, _> = old_instances
            .into_iter()
            .zip(std::mem::take(&mut self.panels))
            .enumerate()
            .map(|(index, (instance, panel))| (instance.key, (index, panel)))
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
            old: &mut HashMap<InstanceKey, (usize, PanelState)>,
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
                old: &mut HashMap<InstanceKey, (usize, PanelState)>,
                panels: &mut Vec<PanelState>,
                remap: &mut HashMap<usize, usize>,
            ) -> usize {
                let key = InstanceKey {
                    template,
                    value: binding.as_ref().map(|(_, v)| v.value.clone()),
                    occurrence,
                };
                let index = panels.len();
                let mut panel = if let Some((old_index, panel)) = old.remove(&key) {
                    remap.insert(old_index, index);
                    panel
                } else {
                    dynamic.templates[template].clone()
                };
                let scope = state.panel_scopes.get(template).copied().unwrap_or(0);
                let mut texts = state.scope_values(scope, vars);
                for name in texts.clone().keys() {
                    if let Some(v) = state.lookup(scope, name) {
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
        self.layout = layout;
        if self.selected_panel_index().is_none() {
            self.mode = match self.mode {
                AppMode::Fullscreen => AppMode::Normal,
                AppMode::FullscreenInspect => AppMode::Inspect,
                mode => mode,
            };
        }
        self.dynamic = Some(dynamic);
    }
}
#[cfg(test)]
mod tests;
