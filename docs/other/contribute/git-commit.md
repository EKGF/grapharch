---
title: Git Commit Notes
parent: Contribute
---

# Git Commit Notes

Write a meaningful commit message &mdash;
compliant with the "conventional commits" standard,
see below &mdash; that explains
what your changes do.

```shell
git add .
git commit -m "feat: add new feature"

# In the commit body or footer, reference the issue:
Closes #<issue-number>
```

See [Contribute](https://ekgf.github.io/GraphArch/contribute/README/)

## Conventional Commits

**Conventional Commits** is a specification for adding human and
machine-readable meaning to commit messages.
It is a lightweight convention on top of commit messages.
It provides an easy set of rules for creating an explicit commit history,
that can then be used to generate changelogs, upgrade notes, and more.

See [Conventional Commits](https://www.conventionalcommits.org/) for more information.

See also [Cocogitto User Guide](https://docs.cocogitto.io/guide/init.html).

We enforce the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
standard by running a "commit hook" that checks your commit messages on your local machine
and also by running a "GitHub Action Workflow" that checks your commit messages on GitHub.

Please use clear and descriptive commit messages that follow this convention.
Additionally, every commit should reference the issue it addresses in the message body or footer,
using `Closes #<issue-number>` or `Fixes #<issue-number>`.

## Why Use Conventional Commits

- Automatically generating CHANGELOGs.
- Automatically determining a semantic version
  bump (based on the types of commits landed).
- Communicating the nature of changes to
  teammates, the public, and other stakeholders.
- Triggering build and publish processes.
- Making it easier for people to contribute to
  your projects, by allowing them to explore a
  more structured commit history.

## Examples of Valid Commit Messages

Examples of valid commit messages:

```text
feat(sparql): add support for SPARQL endpoint

Closes #12
```

```text
fix(docs): correct typo

Fixes #34
```

```text
chore: update dependencies to latest versions

Related to #56
```

- See more examples: https://gist.github.com/qoomon/5dfcdf8eec66a051ecd85625518cfd13

### Push Your Branch

Push your branch to your forked repository.

```shell
git push origin features/your-feature-name
```

## Links

- [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
- [How to install Cocogitto on MacOS](./macos.md#conventional-commits)
