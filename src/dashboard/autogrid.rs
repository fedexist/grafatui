use crate::display_units::{LOGICAL_CELL_HEIGHT_PX, LOGICAL_CELL_WIDTH_PX};

/// Static AutoGrid dimensions normalized to terminal cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutoGridOptions {
    pub(crate) max_columns: usize,
    pub(crate) min_column_width: u32,
    pub(crate) row_height: u32,
    pub(crate) fill_screen: bool,
}

impl Default for AutoGridOptions {
    fn default() -> Self {
        Self {
            fill_screen: false,
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
    pub(crate) panels: Vec<usize>,
}
