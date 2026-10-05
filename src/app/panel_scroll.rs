//! Persistent panel body state, independent of dashboard scrolling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PanelBodyIdentity {
    Table,
    Error(String),
}

#[derive(Debug, Clone)]
pub(crate) struct PanelBodyScroll {
    pub(crate) identity: PanelBodyIdentity,
    pub(crate) offset: u64,
}
