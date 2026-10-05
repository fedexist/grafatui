use crate::display_units::{LOGICAL_CELL_HEIGHT_PX, LOGICAL_CELL_WIDTH_PX};

/// AutoGrid dimensions normalized to terminal cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutoGridOptions {
    pub(crate) max_columns: usize,
    pub(crate) min_column_width: u32,
    pub(crate) row_height: u32,
    pub(crate) fill_screen: bool,
    pub(crate) fit_content: bool,
    /// None inherits row_height; Some(0) removes the fit minimum.
    pub(crate) min_height: Option<u32>,
    pub(crate) match_row_heights: bool,
}

impl Default for AutoGridOptions {
    fn default() -> Self {
        Self {
            fill_screen: false,
            fit_content: false,
            min_height: None,
            match_row_heights: true,
            max_columns: 3,
            min_column_width: 448_u32.div_ceil(LOGICAL_CELL_WIDTH_PX),
            row_height: 320_u32.div_ceil(LOGICAL_CELL_HEIGHT_PX),
        }
    }
}

/// A transparent layout group: panels retain their existing runtime identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DashboardAutoGrid {
    pub(crate) options: AutoGridOptions,
    pub(crate) items: Vec<AutoGridItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutoGridItem {
    pub(crate) index: usize,
    pub(crate) fit_content: Option<bool>,
}

#[cfg(test)]
pub(crate) fn test_items(indices: Vec<usize>) -> Vec<AutoGridItem> {
    indices
        .into_iter()
        .map(|index| AutoGridItem {
            index,
            fit_content: None,
        })
        .collect()
}
