//! The profile's term-table arrangement, read from publication definitions.
use {
    super::{kind::TermKind, pattern::TextPattern, rdf_list::RdfList},
    crate::publication::{
        language_range::LanguageRange,
        validation::unsigned,
        view::RevisionView,
    },
    anyhow::{Result, bail, ensure},
    oxrdf::{NamedNode, Term},
};

/// The most reference sections one term table declares.
const SECTIONS: usize = 16;

/// One titled group of rows, selecting terms by declared RDF type.
#[derive(Clone, Debug)]
pub(crate) struct TableSection {
    pub title:      String,
    order:          u32,
    pub term_types: Vec<NamedNode>,
}

impl TableSection {
    /// Whether a term with these declared types belongs to this section.
    pub fn selects(&self, types: &[NamedNode]) -> bool {
        types.iter().any(|kind| self.term_types.contains(kind))
    }
}

/// A profile's tabular ontology reference (`ga:TermTable`).
#[derive(Clone, Debug)]
pub(crate) struct TermTable {
    pub(crate) node:             NamedNode,
    pub(super) sections:         Vec<TableSection>,
    pub(super) metadata:         Option<Vec<NamedNode>>,
    pub(super) annotations:      Option<Vec<NamedNode>>,
    pub(super) metadata_caption: TextPattern,
    pub(super) details_caption:  TextPattern,
    pub(super) metadata_lead_in: Option<TextPattern>,
    pub(super) details_lead_in:  Option<TextPattern>,
}

impl TermTable {
    /// The term table a profile selects with `ga:termTable`, if any.
    pub(crate) async fn read(
        view: &RevisionView,
        profile: &NamedNode,
    ) -> Result<Option<Self>> {
        let node = match view.optional(profile, "termTable").await? {
            None => return Ok(None),
            Some(Term::NamedNode(node)) => node,
            Some(_) => {
                bail!("profile ga:termTable must identify a ga:TermTable")
            },
        };
        ensure!(
            view.has_type(&node, "TermTable").await?,
            "ga:termTable {node} must be a declared ga:TermTable"
        );
        let mut sections = Vec::new(); // bounded: SECTIONS.
        let mut rows = view.objects(&node, "referenceSection").await?;
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(section) = &row["value"] else {
                bail!("ga:referenceSection must identify a ga:ReferenceSection")
            };
            ensure!(
                sections.len() < SECTIONS,
                "a term table declares at most {SECTIONS} sections"
            );
            sections.push(Self::section(view, section).await?);
        }
        ensure!(
            !sections.is_empty(),
            "term table {node} declares no ga:referenceSection"
        );
        sections.sort_by_key(|section| section.order);
        ensure!(
            sections
                .windows(2)
                .all(|pair| pair[0].order != pair[1].order),
            "term table {node} sections need distinct ga:order values"
        );
        Ok(Some(Self {
            metadata: Self::list(view, &node, "metadataPredicates").await?,
            annotations: Self::list(view, &node, "annotationPredicates")
                .await?,
            metadata_caption: TextPattern::caption(
                &Self::text(view, &node, "metadataCaption").await?,
            )?,
            details_caption: TextPattern::caption(
                &Self::text(view, &node, "detailsCaption").await?,
            )?,
            metadata_lead_in: Self::optional_text(
                view,
                &node,
                "metadataLeadIn",
            )
            .await?
            .map(|text| TextPattern::lead_in(&text))
            .transpose()?,
            details_lead_in: Self::optional_text(view, &node, "detailsLeadIn")
                .await?
                .map(|text| TextPattern::lead_in(&text))
                .transpose()?,
            node,
            sections,
        }))
    }

    async fn section(
        view: &RevisionView,
        node: &NamedNode,
    ) -> Result<TableSection> {
        let english = LanguageRange::english();
        let mut titles = Vec::new(); // bounded: 64 titles.
        let mut rows = view
            .objects_by(
                node,
                &NamedNode::new_unchecked("http://purl.org/dc/terms/title"),
            )
            .await?;
        while let Some(row) = rows.next().await? {
            ensure!(
                titles.len() < 64,
                "{node} exceeds the title budget"
            );
            if let Term::Literal(title) = &row["value"] &&
                english.selects(title)
            {
                titles.push(title.value().to_owned());
            }
        }
        ensure!(
            titles.len() == 1 &&
                !titles[0].trim().is_empty() &&
                titles[0].len() <= 160 &&
                !titles[0].contains(['\n', '\r']),
            "term table section {node} requires one English single-line \
             dct:title"
        );
        let order = unsigned(
            &view.optional(node, "order").await?.ok_or_else(|| {
                anyhow::anyhow!("term table section {node} requires ga:order")
            })?,
            "term table section order",
        )?;
        let mut term_types = Vec::new(); // bounded: SECTIONS term types.
        let mut rows = view.objects(node, "termType").await?;
        while let Some(row) = rows.next().await? {
            let Term::NamedNode(kind) = &row["value"] else {
                bail!("ga:termType must identify an RDF class")
            };
            ensure!(
                term_types.len() < SECTIONS,
                "{node} exceeds the term type budget"
            );
            ensure!(
                TermKind::of(std::slice::from_ref(kind)).is_some(),
                "term table section {node} selects {kind}, which is not an \
                 OWL class, datatype, property or named individual type"
            );
            term_types.push(kind.clone());
        }
        ensure!(
            !term_types.is_empty(),
            "term table section {node} requires ga:termType"
        );
        term_types.sort();
        Ok(TableSection { title: titles.remove(0), order, term_types })
    }

    async fn list(
        view: &RevisionView,
        node: &NamedNode,
        field: &str,
    ) -> Result<Option<Vec<NamedNode>>> {
        let Some(head) = view.optional(node, field).await? else {
            return Ok(None);
        };
        let members = RdfList::new(view, head).resources(field).await?;
        let mut seen = std::collections::HashSet::new(); // bounded: rdf_list::MEMBERS.
        ensure!(
            members.iter().all(|member| seen.insert(member.clone())),
            "ga:{field} lists a predicate more than once"
        );
        Ok(Some(members))
    }

    async fn optional_text(
        view: &RevisionView,
        node: &NamedNode,
        field: &str,
    ) -> Result<Option<String>> {
        match view.optional(node, field).await? {
            None => Ok(None),
            Some(Term::Literal(text)) => Ok(Some(text.value().to_owned())),
            Some(_) => bail!("ga:{field} must be text"),
        }
    }

    async fn text(
        view: &RevisionView,
        node: &NamedNode,
        field: &str,
    ) -> Result<String> {
        Self::optional_text(view, node, field)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("term table {node} requires ga:{field}")
            })
    }

    /// The first section, by order, that selects a term with these types.
    pub(super) fn section_of(&self, types: &[NamedNode]) -> Option<usize> {
        self.sections
            .iter()
            .position(|section| section.selects(types))
    }

    /// The identity of a term table, distinguishing configurations.
    pub(crate) fn identity(&self) -> &str { self.node.as_str() }
}
