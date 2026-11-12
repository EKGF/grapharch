//! PDF conformance standards selected by IRI on a publication profile.
//!
//! Every generated PDF is tagged. A profile can additionally require a
//! standard that Typst enforces while exporting, failing the publication when
//! the document violates it. The requirement is recorded in the generated
//! Typst source, so a book worker compiling a prepared file needs no side
//! channel and the book fingerprint covers the requirement.
use {
    super::{markup::quote, model::Plan},
    anyhow::{Result, anyhow, bail, ensure},
    oxrdf::{NamedNode, Term},
    std::collections::BTreeSet,
    typst::{
        foundations::{Label, Value},
        introspection::{Introspector, MetadataElem},
        utils::PicoStr,
    },
    typst_layout::PagedDocument,
    typst_pdf::{PdfOptions, PdfStandards},
};

/// A supported PDF conformance standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PdfStandard {
    /// ISO 14289-1, universal accessibility.
    Ua1,
}

impl PdfStandard {
    /// Resolve a standard definition by its exact RDF identity.
    pub(crate) fn from_node(node: &NamedNode) -> Result<Self> {
        Self::from_iri(node.as_str())
    }

    fn from_iri(iri: &str) -> Result<Self> {
        match iri {
            "https://ekgf.org/ontology/grapharch#PdfUa1" => Ok(Self::Ua1),
            other => bail!("unsupported PDF standard definition {other}"),
        }
    }

    fn iri(self) -> &'static str {
        match self {
            Self::Ua1 => "https://ekgf.org/ontology/grapharch#PdfUa1",
        }
    }

    fn typst(self) -> typst_pdf::PdfStandard {
        match self {
            Self::Ua1 => typst_pdf::PdfStandard::Ua_1,
        }
    }
}

/// The standards one generated PDF must conform to, in a deterministic order.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct PdfConformance {
    // bounded: a subset of the closed set of supported standards.
    standards: BTreeSet<PdfStandard>,
}

impl PdfConformance {
    /// The label of the metadata element recording the requirement.
    const LABEL: &str = "grapharch-pdf-standards";

    /// Read the standards required by the plan's profile.
    pub(super) async fn read(plan: &Plan) -> Result<Self> {
        let mut standards = BTreeSet::new();
        let mut rows =
            plan.source.objects(&plan.profile, "pdfStandard").await?;
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(node) = &row["value"] else {
                bail!("profile pdfStandard must reference a standard IRI")
            };
            standards.insert(PdfStandard::from_node(node)?);
        }
        Ok(Self { standards })
    }

    /// Typst markup recording the requirement for the PDF exporter. A document
    /// without requirements records none.
    pub(super) fn typst(&self) -> String {
        if self.standards.is_empty() {
            return String::new();
        }
        let iris = self
            .standards
            .iter()
            .map(|standard| format!("{}, ", quote(standard.iri())))
            .collect::<String>();
        format!("#metadata(({iris}))<{}>\n", Self::LABEL)
    }

    /// Recover the requirement recorded in a laid-out document.
    pub(super) fn of(document: &PagedDocument) -> Result<Self> {
        let label = Label::new(PicoStr::intern(Self::LABEL))
            .ok_or_else(|| anyhow!("invalid PDF standards label"))?;
        let introspector = document.introspector();
        let mut standards = BTreeSet::new();
        match introspector.label_count(label) {
            0 => {},
            1 => {
                let element = introspector
                    .query_label(label)
                    .map_err(|error| anyhow!("{error}"))?;
                let metadata = element
                    .to_packed::<MetadataElem>()
                    .ok_or_else(|| anyhow!("PDF standards must be metadata"))?;
                let Value::Array(values) = &metadata.value else {
                    bail!("PDF standards metadata must be an array")
                };
                for value in values.iter() {
                    let Value::Str(iri) = value else {
                        bail!("PDF standards metadata must contain IRIs")
                    };
                    ensure!(
                        standards.insert(PdfStandard::from_iri(iri)?),
                        "PDF standard {} is recorded twice",
                        iri.as_str()
                    );
                }
            },
            _ => bail!("a document records its PDF standards once"),
        }
        Ok(Self { standards })
    }

    /// Export options enforcing the requirement. Tagging stays enabled.
    pub(super) fn options(&self) -> Result<PdfOptions> {
        let standards = self
            .standards
            .iter()
            .map(|standard| standard.typst())
            .collect::<Vec<_>>();
        Ok(PdfOptions {
            standards: PdfStandards::new(&standards).map_err(|error| {
                anyhow!("incompatible PDF standards: {}", error.message())
            })?,
            tagged: true,
            ..PdfOptions::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{PdfConformance, PdfStandard},
        crate::publication::render::engine::{Assets, compile},
        std::{collections::BTreeSet, sync::Arc},
    };

    async fn pdf(source: &str) -> anyhow::Result<Vec<u8>> {
        let root = tempfile::tempdir()?;
        std::fs::write(root.path().join("main.typ"), source)?;
        compile(
            root.path().to_owned(),
            Assets::load().await?,
            Arc::new(tokio::sync::Semaphore::new(1)),
        )
        .await
    }

    fn ua1() -> PdfConformance {
        PdfConformance { standards: BTreeSet::from([PdfStandard::Ua1]) }
    }

    #[tokio::test]
    async fn recorded_pdf_ua_requirement_is_enforced_on_export()
    -> anyhow::Result<()> {
        let document = "#set document(title: \"Conformance\")\n= Chapter\n=== \
                        Skipped section\n";
        assert!(
            pdf(document).await?.starts_with(b"%PDF-"),
            "without a requirement only tagging applies"
        );
        let error = pdf(&format!("{}{document}", ua1().typst()))
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("PDF/UA-1 error"),
            "a violated requirement must fail the publication: {error:#}"
        );
        let conforming = pdf(&format!(
            "{}#set document(title: \"Conformance\")\n= Chapter\n== Section\n",
            ua1().typst()
        ))
        .await?;
        assert!(
            conforming
                .windows(b"<pdfuaid:part>1</pdfuaid:part>".len())
                .any(|window| window == b"<pdfuaid:part>1</pdfuaid:part>")
        );
        Ok(())
    }

    #[test]
    fn requirements_without_standards_add_no_markup() {
        assert_eq!(PdfConformance::default().typst(), "");
        assert_eq!(
            ua1().typst(),
            "#metadata((\"https://ekgf.org/ontology/grapharch#PdfUa1\", \
             ))<grapharch-pdf-standards>\n"
        );
    }
}
