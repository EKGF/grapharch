//! Mock RDF4J repository state shared by connection threads.
use {oxigraph::store::Store, std::collections::HashMap};
pub(super) struct Tx {
    pub(super) store:      Store,
    pub(super) generation: u64,
    pub(super) dirty:      bool,
}
pub(super) struct State {
    pub(super) store:         Store,
    pub(super) generation:    u64,
    pub(super) next:          u64,
    pub(super) transactions:  HashMap<u64, Tx>,
    pub(super) serializable:  bool,
    pub(super) source_status: u16,
    pub(super) source:        String,
    pub(super) bad_location:  bool,
    pub(super) lose_commit:   bool,
    pub(super) source_delay:  u64,
    pub(super) authenticated: bool,
    pub(super) connections:   usize,
    pub(super) requests:      usize,
}
