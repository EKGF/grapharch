//! RDF4J statement reads within a transaction.
use {
    super::{reader::SessionReader, session::RemoteSession},
    crate::publication::{dataset::backend::Quads, sparql::http_error},
    oxrdf::{GraphNameRef, NamedNodeRef, NamedOrBlankNodeRef, TermRef},
    oxrdfio::{RdfFormat, RdfParser},
    std::io,
};

impl RemoteSession {
    pub fn quads_for_pattern(
        &self,
        s: Option<NamedOrBlankNodeRef<'_>>,
        p: Option<NamedNodeRef<'_>>,
        o: Option<TermRef<'_>>,
        g: Option<GraphNameRef<'_>>,
    ) -> Quads<'static> {
        let result = (|| -> io::Result<_> {
            self.flush()?;
            let mut params = Vec::new();
            if let Some(value) = s {
                params.push(("subj", value.to_string()));
            }
            if let Some(value) = p {
                params.push(("pred", value.to_string()));
            }
            if let Some(value) = o {
                params.push(("obj", value.to_string()));
            }
            if let Some(value) = g {
                params.push((
                    "context",
                    if value == GraphNameRef::DefaultGraph {
                        "null".into()
                    } else {
                        value.to_string()
                    },
                ));
            }
            let response = self
                .request("GET")?
                .query(&params)
                .header("Accept", "application/n-quads")
                .send()
                .map_err(http_error)?;
            let reader = SessionReader::expecting(
                response,
                "application/n-quads",
                "RDF4J statements response must preserve contexts as N-Quads",
                self,
            )?;
            Ok(RdfParser::from_format(RdfFormat::NQuads).for_reader(reader))
        })();
        match result {
            Ok(parser) => Box::new(parser.map(|q| q.map_err(io::Error::other))),
            Err(e) => Box::new(std::iter::once(Err(e))),
        }
    }
}
