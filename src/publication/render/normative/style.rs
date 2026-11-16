//! The stylesheet rule for declared normative keywords.
use {
    super::{super::model::Plan, Conformance},
    crate::publication::theme::{Builtin, PdfTheme},
    anyhow::Result,
};

impl Conformance {
    /// The `.rfc2119` rule of the profile's theme, or nothing without a
    /// declaration. `default` is the catalogue theme the renderer uses when the
    /// profile selects none.
    pub(in crate::publication::render) async fn css(
        self,
        plan: &Plan,
        default: Builtin,
    ) -> Result<&'static str> {
        if !self.bcp14() {
            return Ok("");
        }
        Ok(
            PdfTheme::read(&plan.source, &plan.profile, default)
                .await?
                .keyword_style()?
                .css(),
        )
    }
}
