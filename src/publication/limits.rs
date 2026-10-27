//! Coordinated finite budgets for large ontology-family publications.
//! Keep resources, fragments, links and output content in the same size class:
//! increasing one stage must not merely move the failure to the next stage.
pub(crate) const FAMILY_RESOURCES: usize = 32_768;
pub(crate) const PUBLICATION_ENTRIES: u64 = 65_536;
pub(crate) const PUBLICATION_CONTENT_BYTES: u64 = 128 * 1024 * 1024;
pub(crate) const LINK_INDEX_ENTRIES: usize = 131_072;
pub(crate) const SPECIFICATION_PAGES: usize = 8192;
pub(crate) const PUBLICATION_FILES: usize = SPECIFICATION_PAGES * 5 + 260;
pub(crate) const SPECIFICATION_ANCHORS: usize = 262_144;
pub(crate) const SPECIFICATION_LINKS: usize = 1_048_576;
pub(crate) const TYPST_SOURCE_BYTES: u64 = 256 * 1024 * 1024;
pub(crate) const ARTIFACT_BYTES: u64 = 128 * 1024 * 1024;
pub(crate) const RETAINED_ASSET_BYTES: u64 = 512 * 1024 * 1024;
pub(crate) const REFERENCE_INDEX_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const SOURCE_TYPE_ASSERTIONS: usize = 262_144;

// Fail compilation when a stage budget becomes smaller than the structures
// that an accepted family can generate.
const _: () = {
    assert!(FAMILY_RESOURCES >= 16_283 + 296);
    assert!(PUBLICATION_ENTRIES as usize >= FAMILY_RESOURCES * 2);
    assert!(LINK_INDEX_ENTRIES >= PUBLICATION_ENTRIES as usize * 2);
    assert!(SPECIFICATION_ANCHORS >= PUBLICATION_ENTRIES as usize * 4);
    assert!(SPECIFICATION_LINKS >= SPECIFICATION_ANCHORS * 4);
};
