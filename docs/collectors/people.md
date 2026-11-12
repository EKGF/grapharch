---
title: Publication People
parent: Collectors
nav_order: 15
permalink: /collectors/people/
grapharch:
  subject: urn:grapharch:docs:collectors:people
---
# Publication People Collectors

Two collectors resolve the people credited in a specification header to their
public records. Neither runs unless a project declares a `ga:CollectorUse` for
it, so no build contacts ORCID or GitHub implicitly. Both run after the source
collectors, because the references they resolve are collected source
assertions. Renderers read only the retained records: a restored KG renders the
same header without network access.

They support the [publication metadata](../documentable-things/README.md) of
specification and PDF targets; they do not discover ontology terms.

## ORCID person collector

`ga:OrcidPersonCollector` finds every ORCID iD IRI
(`http://orcid.org/…` or `https://orcid.org/…`) used as a value of a listed
`ga:personPredicate`, on an ontology header or on any other collected subject.
Both spellings are one person, identified by the canonical
`https://orcid.org/<id>` IRI. The ISO 7064 MOD 11-2 check character is
validated before any request; an invalid iD fails its record without contacting
the API. Each person is fetched once from `<source><id>/person` with
`Accept: application/json`. The ORCID public API needs no token for public data.

Retained per person: `dcterms:title` and `schema:name` (the credit name, else
given and family names), `schema:givenName`, `schema:familyName`, `ga:orcid`,
`ga:url`, the record's name modification time as `dcterms:modified`, the API URL
as `prov:wasDerivedFrom`, and `schema:sameAs` for researcher URLs that are
GitHub profiles. Emails, biographies, addresses and other URLs are not
retained. The contribution revision is a hash of the response.

## GitHub contributor collector

`ga:GitHubContributorCollector` reads the Git provenance of collected GitHub
repositories, walks up to 1,000 commits from the collected commit through
`repos/<owner>/<name>/commits?sha=<commit>`, and counts commit authors with a
linked user account. Commits without an account and bot accounts are counted as
`ga:unattributedCommitCount`. The 32 most active accounts (most commits, then
login) are fetched from `users/<login>`. Retained per account: login
(`foaf:accountName`), display name (or the login), and profile URL
`https://github.com/<login>`; email addresses are never retained. The repository
records `ga:examinedCommit`, `ga:examinedCommitCount`, `ga:contributorCount` and
`ga:commitHistoryComplete`. `GITHUB_TOKEN` is sent only when the source is
`https://api.github.com/`; without it the unauthenticated rate limit applies.

A GitHub account is shown as an ORCID person only when that person's ORCID
record lists the profile URL. No other linkage, such as matching names, is
inferred.

## Enable and credit people

```turtle
@prefix dc: <http://purl.org/dc/elements/1.1/> .
<urn:example:project> ga:collector <urn:example:orcid>, <urn:example:github> .
<urn:example:orcid> a ga:CollectorUse ; dct:title "ORCID people" ;
    ga:collectorType ga:OrcidPersonCollector ;
    ga:source "https://pub.orcid.org/v3.0/" ;
    ga:personPredicate dc:creator, dct:creator, dct:contributor .
<urn:example:github> a ga:CollectorUse ; dct:title "GitHub contributors" ;
    ga:collectorType ga:GitHubContributorCollector ;
    ga:source "https://api.github.com/" .

<urn:example:specification> ga:credit <urn:example:editors>, <urn:example:contributors> .
<urn:example:editors> a ga:Credit ;
    ga:creditRole ga:editor ; ga:creditPredicate dc:creator .
<urn:example:contributors> a ga:Credit ;
    ga:creditRole ga:contributor ;
    ga:creditPredicate <https://schema.org/contributor> .
```

A `ga:Credit` lists every IRI value of its predicate under `ga:editor`,
`ga:author`, `ga:formerEditor` or `ga:contributor`. The header uses the role
labels Editors, Authors, Former editors and Contributors; a target with
`ga:metadataRow`s adds a row with `ga:field "editor"` or `"contributor"` and its
own `dcterms:title`. Explicit people come first, then credited people by commit
count and surname. A person is listed under the first role that credits them.
A credited value without a retained name, or a literal value, is an error.

## Bounds and failures

At most 64 people per ORCID use and eight repository revisions per GitHub use;
responses are limited to 256 KiB (ORCID), 4 MiB per commit page and 64 KiB per
GitHub profile, with a 10 s connect and 30 s request timeout and no redirects.
A failed or unsuccessful request fails that record, records a
`collection-failed` diagnostic against its source in the KG, and fails the
build; it never becomes an empty list. The source must be HTTPS; plain HTTP is
accepted only for a loopback mirror.
