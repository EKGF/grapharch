use {ecow::EcoVec, typst::diag::SourceDiagnostic};

#[derive(Debug)]
pub struct TypstError(pub EcoVec<SourceDiagnostic>);

impl std::fmt::Display for TypstError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for diagnostic in &self.0 {
            writeln!(f, "{:?}", diagnostic)?;
        }
        Ok(())
    }
}

impl std::error::Error for TypstError {}

// Instead of implementing From, create a conversion function
#[allow(dead_code)]
pub fn into_anyhow(err: TypstError) -> anyhow::Error { anyhow::Error::new(err) }

// Conversion function that properly handles EcoVec<SourceDiagnostic>
pub fn eco_vec_into_anyhow(err: EcoVec<SourceDiagnostic>) -> anyhow::Error {
    anyhow::Error::new(TypstError(err))
}
