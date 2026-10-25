//! Common stage operations. Remote transactions never mirror data into a local
//! store.
use {
    oxigraph::{
        sparql::{QueryResults, SparqlEvaluator},
        store::{StorageError, Store, Transaction as EmbeddedTransaction},
    },
    oxrdf::{
        GraphNameRef,
        NamedNodeRef,
        NamedOrBlankNodeRef,
        Quad,
        QuadRef,
        TermRef,
    },
    std::{
        io::{self, Write},
        sync::Arc,
    },
};
pub(crate) mod remote;
pub use remote::SparqlStoreConfig;
pub(crate) enum BackendStore {
    Embedded(Store),
    Remote(Arc<remote::RemoteStore>),
}
pub(crate) enum Transaction<'a, 'store> {
    Embedded(&'a mut EmbeddedTransaction<'store>),
    Remote(remote::RemoteSession),
}
pub(crate) type Quads<'a> = Box<dyn Iterator<Item = io::Result<Quad>> + 'a>;
fn error(error: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::other(error)
}
impl BackendStore {
    pub fn memory() -> io::Result<Self> {
        Ok(Self::Embedded(Store::new().map_err(error)?))
    }

    pub fn open(path: std::path::PathBuf) -> io::Result<Self> {
        Ok(Self::Embedded(Store::open(path).map_err(error)?))
    }

    pub fn query(&self, query: &str) -> io::Result<QueryResults<'static>> {
        match self {
            Self::Embedded(s) => {
                SparqlEvaluator::new()
                    .parse_query(query)
                    .map_err(error)?
                    .on_store(s)
                    .execute()
                    .map_err(error)
            },
            Self::Remote(s) => s.session()?.query(query),
        }
    }

    pub fn update(&self, query: &str) -> io::Result<()> {
        match self {
            Self::Embedded(s) => s.update(query).map_err(error),
            Self::Remote(s) => {
                let session = s.session()?;
                match session.update(query) {
                    Ok(()) => session.commit(),
                    Err(e) => {
                        session.rollback()?;
                        Err(e)
                    },
                }
            },
        }
    }

    pub fn contains(&self, quad: QuadRef<'_>) -> io::Result<bool> {
        match self {
            Self::Embedded(s) => s.contains(quad).map_err(error),
            Self::Remote(s) => {
                let session = s.session()?;
                let found = session
                    .quads_for_pattern(
                        Some(quad.subject),
                        Some(quad.predicate),
                        Some(quad.object),
                        Some(quad.graph_name),
                    )
                    .next()
                    .transpose()?
                    .is_some();
                session.rollback()?;
                Ok(found)
            },
        }
    }

    pub fn insert(&self, quad: QuadRef<'_>) -> io::Result<()> {
        match self {
            Self::Embedded(s) => s.insert(quad).map_err(error),
            Self::Remote(s) => {
                let session = s.session()?;
                session.insert(quad)?;
                session.commit()?;
                Ok(())
            },
        }
    }

    pub fn is_empty(&self) -> io::Result<bool> {
        match self {
            Self::Embedded(s) => s.is_empty().map_err(error),
            Self::Remote(s) => {
                let session = s.session()?;
                let empty = session
                    .quads_for_pattern(None, None, None, None)
                    .next()
                    .transpose()?
                    .is_none();
                session.rollback()?;
                Ok(empty)
            },
        }
    }

    pub fn quads_for_pattern(
        &self,
        s: Option<NamedOrBlankNodeRef<'_>>,
        p: Option<NamedNodeRef<'_>>,
        o: Option<TermRef<'_>>,
        g: Option<GraphNameRef<'_>>,
    ) -> Quads<'_> {
        match self {
            Self::Embedded(store) => {
                Box::new(
                    store
                        .quads_for_pattern(s, p, o, g)
                        .map(|r| r.map_err(error)),
                )
            },
            Self::Remote(store) => {
                match store.session() {
                    Ok(session) => session.quads_for_pattern(s, p, o, g),
                    Err(e) => Box::new(std::iter::once(Err(e))),
                }
            },
        }
    }

    pub fn transaction<T, E>(
        &self,
        f: impl for<'a, 'store> Fn(Transaction<'a, 'store>) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: std::error::Error + 'static + From<StorageError> + From<io::Error>,
    {
        match self {
            Self::Embedded(s) => {
                let mut tx = s.start_transaction().map_err(E::from)?;
                let value = f(Transaction::Embedded(&mut tx))?;
                tx.commit().map_err(E::from)?;
                Ok(value)
            },
            Self::Remote(s) => {
                let session = s.session().map_err(E::from)?;
                match f(Transaction::Remote(session.clone())) {
                    Ok(value) => {
                        session.commit().map_err(E::from)?;
                        Ok(value)
                    },
                    Err(error) => {
                        session.rollback().map_err(E::from)?;
                        Err(error)
                    },
                }
            },
        }
    }

    pub fn dump_to_writer<W: Write>(
        &self,
        format: oxrdfio::RdfFormat,
        writer: W,
    ) -> io::Result<W> {
        match self {
            Self::Embedded(s) => {
                s.dump_to_writer(format, writer).map_err(error)
            },
            Self::Remote(s) => {
                if format != oxrdfio::RdfFormat::NQuads {
                    return Err(io::Error::other(
                        "external dataset export requires N-Quads",
                    ));
                }
                let session = s.session()?;
                let mut serializer =
                    oxrdfio::RdfSerializer::from_format(format)
                        .for_writer(writer);
                for quad in session.quads_for_pattern(None, None, None, None) {
                    serializer.serialize_quad(&quad?)?;
                }
                let writer = serializer.finish()?;
                session.rollback()?;
                Ok(writer)
            },
        }
    }
}
impl Transaction<'_, '_> {
    pub fn query(&self, query: &str) -> io::Result<QueryResults<'_>> {
        match self {
            Self::Embedded(s) => {
                SparqlEvaluator::new()
                    .parse_query(query)
                    .map_err(error)?
                    .on_transaction(s)
                    .execute()
                    .map_err(error)
            },
            Self::Remote(s) => s.query(query),
        }
    }

    pub fn update(&mut self, query: &str) -> io::Result<()> {
        match self {
            Self::Embedded(s) => s.update(query).map_err(error),
            Self::Remote(s) => s.update(query),
        }
    }

    pub fn insert(&mut self, quad: QuadRef<'_>) -> io::Result<()> {
        match self {
            Self::Embedded(s) => {
                s.insert(quad);
                Ok(())
            },
            Self::Remote(s) => {
                s.insert(quad)?;
                Ok(())
            },
        }
    }

    pub fn contains(&self, quad: QuadRef<'_>) -> io::Result<bool> {
        match self {
            Self::Embedded(s) => s.contains(quad).map_err(error),
            Self::Remote(s) => {
                Ok(s.quads_for_pattern(
                    Some(quad.subject),
                    Some(quad.predicate),
                    Some(quad.object),
                    Some(quad.graph_name),
                )
                .next()
                .transpose()?
                .is_some())
            },
        }
    }

    pub fn remove(&mut self, quad: QuadRef<'_>) -> io::Result<()> {
        match self {
            Self::Embedded(s) => {
                s.remove(quad);
                Ok(())
            },
            Self::Remote(s) => {
                s.update(&format!(
                    "DELETE DATA {{ {} }}",
                    remote::quad_pattern(quad)
                ))?;
                Ok(())
            },
        }
    }

    pub fn remove_named_graph(
        &mut self,
        graph: NamedNodeRef<'_>,
    ) -> io::Result<()> {
        match self {
            Self::Embedded(s) => s.remove_named_graph(graph).map_err(error),
            Self::Remote(s) => {
                s.update(&format!("DROP SILENT GRAPH {graph}"))?;
                Ok(())
            },
        }
    }

    pub fn quads_for_pattern(
        &self,
        s: Option<NamedOrBlankNodeRef<'_>>,
        p: Option<NamedNodeRef<'_>>,
        o: Option<TermRef<'_>>,
        g: Option<GraphNameRef<'_>>,
    ) -> Quads<'_> {
        match self {
            Self::Embedded(store) => {
                Box::new(
                    store
                        .quads_for_pattern(s, p, o, g)
                        .map(|r| r.map_err(error)),
                )
            },
            Self::Remote(store) => store.quads_for_pattern(s, p, o, g),
        }
    }
}

#[cfg(test)]
mod migration_tests {
    use {
        super::*,
        oxrdf::{GraphName, NamedNode},
    };

    #[test]
    fn transactional_streams_read_staged_writes_and_abort_without_promoting()
    -> io::Result<()> {
        let store = BackendStore::memory()?;
        let quad = Quad::new(
            NamedNode::new_unchecked("urn:test:subject"),
            NamedNode::new_unchecked("urn:test:predicate"),
            NamedNode::new_unchecked("urn:test:object"),
            GraphName::DefaultGraph,
        );
        let result = store.transaction::<(), io::Error>(|mut tx| {
            tx.insert(quad.as_ref())?;
            let QueryResults::Solutions(mut rows) = tx.query(
                "SELECT ?object WHERE { <urn:test:subject> \
                 <urn:test:predicate> ?object }",
            )?
            else {
                panic!("expected SELECT")
            };
            assert_eq!(
                rows.next().transpose().map_err(error)?.unwrap()["object"],
                quad.object
            );
            assert!(rows.next().is_none());
            drop(rows);
            tx.remove(quad.as_ref())?;
            tx.insert(quad.as_ref())?;
            Err(io::Error::other("abort staged operation"))
        });
        assert!(result.is_err());
        assert!(store.is_empty()?);
        store.transaction::<_, io::Error>(|mut tx| tx.insert(quad.as_ref()))?;
        assert!(store.contains(quad.as_ref())?);
        Ok(())
    }
}
