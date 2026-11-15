//! The merged statements of an RDF target's sources, in deterministic order,
//! serialized once per declared format.
use {
    super::{format::Serialization, source_text::SourceText},
    anyhow::{Context, Result, ensure},
    oxrdf::Quad,
    oxrdfio::RdfSerializer,
    std::collections::{BTreeMap, HashSet},
};

pub(super) struct MergedSources {
    quads:    Vec<Quad>, // bounded: source_text::MAX_QUADS.
    prefixes: BTreeMap<String, String>,
}

impl MergedSources {
    /// Parse every source into one graph of distinct statements.
    pub(super) fn parse(sources: &[SourceText]) -> Result<Self> {
        let mut quads = HashSet::new();
        let mut prefixes = BTreeMap::new();
        for source in sources {
            source.parse(&mut quads, &mut prefixes)?;
        }
        // Deterministic output: statements in lexical order.
        let mut quads = quads.into_iter().collect::<Vec<_>>();
        quads.sort_by_cached_key(ToString::to_string);
        Ok(Self { quads, prefixes })
    }

    /// Serialize the merged statements in one declared format.
    pub(super) fn serialize(
        &self,
        serialization: &Serialization,
    ) -> Result<Vec<u8>> {
        ensure!(
            serialization.holds_datasets() ||
                self.quads
                    .iter()
                    .all(|quad| quad.graph_name.is_default_graph()),
            "{} ({}) cannot represent the named graphs of the merged sources",
            serialization.iri,
            serialization.media_type
        );
        let mut serializer = RdfSerializer::from_format(serialization.syntax);
        for (prefix, iri) in &self.prefixes {
            serializer = serializer
                .with_prefix(prefix.clone(), iri.clone())
                .with_context(|| format!("invalid prefix {prefix}"))?;
        }
        let mut writer = serializer.for_writer(Vec::new());
        for quad in &self.quads {
            writer.serialize_quad(quad)?;
        }
        Ok(writer.finish()?)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::publication::rdf_target::names::{FileExtension, MediaType},
        oxrdf::NamedNode,
        oxrdfio::RdfFormat,
    };

    fn serialization(media_type: &str) -> Result<Serialization> {
        Ok(Serialization {
            iri:        NamedNode::new("urn:example:format")?,
            media_type: MediaType::try_new(media_type)?,
            extension:  FileExtension::try_new("x")?,
            syntax:     RdfFormat::from_media_type(media_type).unwrap(),
        })
    }

    #[test]
    fn only_dataset_formats_publish_named_graphs() -> Result<()> {
        let source = SourceText::new(
            "<urn:s> <urn:p> <urn:o> <urn:g> .\n<urn:s> <urn:p> <urn:o> .\n"
                .into(),
            "application/n-quads",
            "https://example.org/",
        )?;
        let merged = MergedSources::parse(&[source])?;
        for media_type in [
            "text/turtle",
            "application/n-triples",
            "application/rdf+xml",
        ] {
            let error = merged
                .serialize(&serialization(media_type)?)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("cannot represent the named graphs"),
                "{error}"
            );
        }
        for media_type in [
            "application/n-quads",
            "application/trig",
            "application/ld+json",
        ] {
            let bytes = merged.serialize(&serialization(media_type)?)?;
            let parsed = oxrdfio::RdfParser::from_format(
                RdfFormat::from_media_type(media_type).unwrap(),
            )
            .for_slice(&bytes)
            .collect::<Result<Vec<_>, _>>()?;
            assert_eq!(parsed.len(), 2, "{media_type}");
        }
        Ok(())
    }
}
