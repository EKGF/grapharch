//! Canonical collector definitions, distinct from source-bound uses.
use {
    anyhow::{Result, bail},
    oxrdf::NamedNode,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CollectorKind {
    RdfFile,
    RdfUrl,
    GitRdfFile,
    AbbreviationFile,
    AuthoredUseCases,
    RepositoryPublications,
    DocumentationNavigation,
    PlantUmlFile,
    MarkdownFile,
    SparqlFile,
    W3cPublishedSpecification,
    Image,
    CodeFile,
    MarkdownPage,
    OrcidPerson,
    GitHubContributor,
    HtmlPreamble,
}
impl CollectorKind {
    pub(crate) fn from_node(node: &NamedNode) -> Result<Self> {
        match node.as_str() {
            "https://ekgf.org/ontology/grapharch#RdfFileCollector" => Ok(Self::RdfFile),
            "https://ekgf.org/ontology/grapharch#RdfUrlCollector" => Ok(Self::RdfUrl),
            "https://ekgf.org/ontology/grapharch#GitRdfFileCollector" => Ok(Self::GitRdfFile),
            "https://ekgf.org/ontology/grapharch#AbbreviationFileCollector" => Ok(Self::AbbreviationFile),
            "https://ekgf.org/ontology/grapharch#AuthoredUseCasesCollector" => Ok(Self::AuthoredUseCases),
            "https://ekgf.org/ontology/grapharch#RepositoryPublicationsCollector" => Ok(Self::RepositoryPublications),
            "https://ekgf.org/ontology/grapharch#DocumentationNavigationCollector" => Ok(Self::DocumentationNavigation),
            "https://ekgf.org/ontology/grapharch#PlantUmlFileCollector" => Ok(Self::PlantUmlFile),
            "https://ekgf.org/ontology/grapharch#MarkdownFileCollector" => Ok(Self::MarkdownFile),
            "https://ekgf.org/ontology/grapharch#SparqlFileCollector" => Ok(Self::SparqlFile),
            "https://ekgf.org/ontology/grapharch#W3cPublishedSpecificationCollector" => Ok(Self::W3cPublishedSpecification),
            "https://ekgf.org/ontology/grapharch#ImageCollector" => Ok(Self::Image),
            "https://ekgf.org/ontology/grapharch#CodeFileCollector" => Ok(Self::CodeFile),
            "https://ekgf.org/ontology/grapharch#MarkdownPageCollector" => Ok(Self::MarkdownPage),
            "https://ekgf.org/ontology/grapharch#OrcidPersonCollector" => Ok(Self::OrcidPerson),
            "https://ekgf.org/ontology/grapharch#GitHubContributorCollector" => Ok(Self::GitHubContributor),
            "https://ekgf.org/ontology/grapharch#HtmlPreambleCollector" => Ok(Self::HtmlPreamble),
            _=>bail!("unsupported collectorType IRI {node}"),
        }
    }

    /// Person collectors read references from already collected
    /// contributions, so they run after the source collectors of a build.
    pub(crate) fn reads_collected_people(self) -> bool {
        matches!(self, Self::OrcidPerson | Self::GitHubContributor)
    }
}
