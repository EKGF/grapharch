---
title: Source
layout: default
parent: Features
nav_order: 2
---

# Sources

GraphArch supports multiple sources that can be scanned for "documentable items" such as
OWL ontologies, SHACL shapes or anything else.

There are generally two types of sources:

- **File-based sources**, these are sources where data is stored in files such as
  the local file system or a remote file system like NFS or object storage like S3.
  Another example of a remote file-oriented source is a remote git repository.
- **Non-file-based sources** such as a graph database.
