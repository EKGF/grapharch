# Onboard Graph Engine

GraphArch includes [OxiGraph](https://github.com/oxigraph/oxigraph),
a graph database that can be used to generate data for further documentation,
integrating both input options seamlessly.

>[!NOTE]
>It may be necessary, as a feature, to support other graph databases
>as well for this intermediate processing.
>[RDFox](https://www.oxfordsemantic.tech) would be particularly useful
>since it can be baked into the Rust program that we intend to build and
>has ultimate speed, OWL reasoning and rule capabilities (SHACL, Datalog).

PS: This is not really a feature but more like a technical design decision.

## Derived features

- Support other databases as the intermediate store, not just embedded
  databases like OxiGraph or RDFox but external databases, probably
  most realistic to narrow that down to SPARQL databases.
  Note that this intermediate store is NOT necessarily the source of
  the documentation elements but in case the source is a SPARQL database
  that GraphArch has write-access to, then it could also serve as the
  intermediate database.
