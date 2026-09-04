#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RowId(usize);

impl RowId {
    pub(crate) const fn new(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DashboardItemId {
    Row(RowId),
    Panel(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DashboardLayoutItem {
    Row(DashboardRow),
    Panel(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DashboardRow {
    pub(crate) id: RowId,
    pub(crate) title: String,
    pub(crate) collapsed: bool,
    pub(crate) hidden_header: bool,
    pub(crate) children: Vec<DashboardLayoutItem>,
}

impl DashboardRow {
    pub(crate) fn new(
        id: RowId,
        title: impl Into<String>,
        collapsed: bool,
        hidden_header: bool,
        children: Vec<DashboardLayoutItem>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            collapsed,
            hidden_header,
            children,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VisibleDashboardItem {
    pub(crate) id: DashboardItemId,
    pub(crate) depth: usize,
}

impl VisibleDashboardItem {
    pub(crate) const fn panel(index: usize, depth: usize) -> Self {
        Self {
            id: DashboardItemId::Panel(index),
            depth,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DashboardLayout {
    pub(crate) items: Vec<DashboardLayoutItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LayoutChange {
    pub(crate) newly_visible_panels: Vec<usize>,
}

impl DashboardLayout {
    pub(crate) fn new(items: Vec<DashboardLayoutItem>) -> Self {
        Self { items }
    }

    pub(crate) fn flat(panel_count: usize) -> Self {
        Self::new((0..panel_count).map(DashboardLayoutItem::Panel).collect())
    }

    pub(crate) fn row(&self, id: RowId) -> Option<&DashboardRow> {
        find_row(&self.items, id)
    }

    pub(crate) fn visible_items(&self) -> Vec<VisibleDashboardItem> {
        let mut visible = Vec::new();
        collect_visible_items(&self.items, 0, &mut visible);
        visible
    }

    pub(crate) fn visible_panel_indices(&self) -> Vec<usize> {
        self.visible_items()
            .into_iter()
            .filter_map(|item| match item.id {
                DashboardItemId::Panel(index) => Some(index),
                DashboardItemId::Row(_) => None,
            })
            .collect()
    }

    pub(crate) fn visible_panel_count(&self) -> usize {
        self.visible_panel_indices().len()
    }

    pub(crate) fn toggle_row(&mut self, id: RowId) -> Option<LayoutChange> {
        let collapsed = !self.row(id)?.collapsed;
        self.set_row_collapsed(id, collapsed)
    }

    pub(crate) fn set_row_collapsed(&mut self, id: RowId, collapsed: bool) -> Option<LayoutChange> {
        let before = self.visible_panel_indices();
        find_row_mut(&mut self.items, id)?.collapsed = collapsed;
        let after = self.visible_panel_indices();
        Some(LayoutChange {
            newly_visible_panels: after
                .into_iter()
                .filter(|panel| !before.contains(panel))
                .collect(),
        })
    }

    pub(crate) fn first_visible(&self) -> Option<DashboardItemId> {
        self.visible_items().first().map(|item| item.id)
    }

    pub(crate) fn nearest_visible_ancestor(&self, id: DashboardItemId) -> Option<DashboardItemId> {
        let visible = self.visible_items();
        if visible.iter().any(|item| item.id == id) {
            return Some(id);
        }

        let mut ancestors = Vec::new();
        if find_ancestors(&self.items, id, &mut ancestors) {
            for row_id in ancestors.into_iter().rev() {
                let ancestor = DashboardItemId::Row(row_id);
                if visible.iter().any(|item| item.id == ancestor) {
                    return Some(ancestor);
                }
            }
        }

        self.first_visible()
    }
}

fn find_row(items: &[DashboardLayoutItem], id: RowId) -> Option<&DashboardRow> {
    for item in items {
        if let DashboardLayoutItem::Row(row) = item {
            if row.id == id {
                return Some(row);
            }
            if let Some(found) = find_row(&row.children, id) {
                return Some(found);
            }
        }
    }
    None
}

fn find_row_mut(items: &mut [DashboardLayoutItem], id: RowId) -> Option<&mut DashboardRow> {
    for item in items {
        if let DashboardLayoutItem::Row(row) = item {
            if row.id == id {
                return Some(row);
            }
            if let Some(found) = find_row_mut(&mut row.children, id) {
                return Some(found);
            }
        }
    }
    None
}

fn collect_visible_items(
    items: &[DashboardLayoutItem],
    depth: usize,
    visible: &mut Vec<VisibleDashboardItem>,
) {
    for item in items {
        match item {
            DashboardLayoutItem::Panel(index) => {
                visible.push(VisibleDashboardItem::panel(*index, depth))
            }
            DashboardLayoutItem::Row(row) if row.hidden_header => {
                collect_visible_items(&row.children, depth, visible);
            }
            DashboardLayoutItem::Row(row) => {
                visible.push(VisibleDashboardItem {
                    id: DashboardItemId::Row(row.id),
                    depth,
                });
                if !row.collapsed {
                    collect_visible_items(&row.children, depth + 1, visible);
                }
            }
        }
    }
}

fn find_ancestors(
    items: &[DashboardLayoutItem],
    target: DashboardItemId,
    ancestors: &mut Vec<RowId>,
) -> bool {
    for item in items {
        match item {
            DashboardLayoutItem::Panel(index) if target == DashboardItemId::Panel(*index) => {
                return true;
            }
            DashboardLayoutItem::Row(row) => {
                if target == DashboardItemId::Row(row.id) {
                    return true;
                }
                ancestors.push(row.id);
                if find_ancestors(&row.children, target, ancestors) {
                    return true;
                }
                ancestors.pop();
            }
            DashboardLayoutItem::Panel(_) => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapsed_parent_hides_descendants_and_preserves_nested_state() {
        let nested = DashboardRow::new(
            RowId::new(2),
            "Nested",
            true,
            false,
            vec![DashboardLayoutItem::Panel(2)],
        );
        let parent = DashboardRow::new(
            RowId::new(1),
            "Parent",
            false,
            false,
            vec![
                DashboardLayoutItem::Panel(0),
                DashboardLayoutItem::Row(nested),
                DashboardLayoutItem::Panel(1),
            ],
        );
        let mut layout = DashboardLayout::new(vec![DashboardLayoutItem::Row(parent)]);

        assert_eq!(layout.visible_panel_indices(), vec![0, 1]);
        layout.set_row_collapsed(RowId::new(1), true).unwrap();
        assert_eq!(layout.visible_panel_indices(), Vec::<usize>::new());
        let change = layout.set_row_collapsed(RowId::new(1), false).unwrap();
        assert_eq!(change.newly_visible_panels, vec![0, 1]);
        assert!(layout.row(RowId::new(2)).unwrap().collapsed);
    }

    #[test]
    fn hidden_header_is_transparent_even_when_marked_collapsed() {
        let hidden = DashboardRow::new(
            RowId::new(1),
            "Hidden",
            true,
            true,
            vec![DashboardLayoutItem::Panel(0)],
        );
        let layout = DashboardLayout::new(vec![DashboardLayoutItem::Row(hidden)]);
        assert_eq!(layout.visible_panel_indices(), vec![0]);
        assert_eq!(
            layout.visible_items(),
            vec![VisibleDashboardItem::panel(0, 0)]
        );
    }

    #[test]
    fn flat_layout_lists_each_panel_at_root_depth() {
        let layout = DashboardLayout::flat(3);

        assert_eq!(layout.visible_panel_count(), 3);
        assert_eq!(
            layout.visible_items(),
            vec![
                VisibleDashboardItem::panel(0, 0),
                VisibleDashboardItem::panel(1, 0),
                VisibleDashboardItem::panel(2, 0),
            ]
        );
        assert_eq!(layout.first_visible(), Some(DashboardItemId::Panel(0)));
    }

    #[test]
    fn toggling_a_collapsed_row_reveals_panels_in_source_order() {
        let row = DashboardRow::new(
            RowId::new(1),
            "Collapsed",
            true,
            false,
            vec![DashboardLayoutItem::Panel(3), DashboardLayoutItem::Panel(1)],
        );
        let mut layout = DashboardLayout::new(vec![DashboardLayoutItem::Row(row)]);

        let change = layout.toggle_row(RowId::new(1)).unwrap();

        assert_eq!(change.newly_visible_panels, vec![3, 1]);
        assert_eq!(layout.visible_panel_indices(), vec![3, 1]);
    }

    #[test]
    fn unknown_rows_do_not_change_the_layout() {
        let mut layout = DashboardLayout::flat(1);

        assert_eq!(layout.set_row_collapsed(RowId::new(99), true), None);
        assert_eq!(layout.toggle_row(RowId::new(99)), None);
        assert_eq!(layout.visible_panel_indices(), vec![0]);
    }

    #[test]
    fn nearest_visible_ancestor_prefers_the_target_then_visible_rows_then_first_item() {
        let collapsed_child = DashboardRow::new(
            RowId::new(2),
            "Child",
            true,
            false,
            vec![DashboardLayoutItem::Panel(2)],
        );
        let parent = DashboardRow::new(
            RowId::new(1),
            "Parent",
            false,
            false,
            vec![DashboardLayoutItem::Row(collapsed_child)],
        );
        let layout = DashboardLayout::new(vec![
            DashboardLayoutItem::Panel(0),
            DashboardLayoutItem::Row(parent),
        ]);

        assert_eq!(
            layout.nearest_visible_ancestor(DashboardItemId::Panel(0)),
            Some(DashboardItemId::Panel(0))
        );
        assert_eq!(
            layout.nearest_visible_ancestor(DashboardItemId::Panel(2)),
            Some(DashboardItemId::Row(RowId::new(2)))
        );
        assert_eq!(
            layout.nearest_visible_ancestor(DashboardItemId::Panel(99)),
            Some(DashboardItemId::Panel(0))
        );
    }
}
