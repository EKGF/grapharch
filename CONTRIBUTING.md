# Contributing to GraphArch

Thank you for considering contributing to **GraphArch**!

We appreciate your interest in improving this
project, and we're excited to work with you.
This document provides guidelines to help you
get started with contributing to GraphArch.

## How Can You Contribute?

There are several ways you can contribute to GraphArch:

1. **Report Bugs**:

   If you come across a bug, please create an
   issue describing the problem and how to
   reproduce it.

2. **Suggest New Features**:

   If you have an idea for a new feature or an
   enhancement, feel free to open an issue to
   discuss it.

3. **Code Contributions**:

   You can contribute to the development of
   GraphArch by fixing bugs,
   adding new features,
   or improving the documentation.

4. **Improve Documentation**:

   Documentation is key to making GraphArch
   accessible.
   You can help by improving existing
   documentation or writing new guides.

## Contributor License Agreement (CLA)

To contribute to this project, you must sign
the Contributor License Agreement (CLA).
We use [CLA assistant](https://cla-assistant.io/) to streamline this process.
You will be prompted to sign the CLA when you
make your first contribution.

## Getting Started (as a developer)

### Create an Issue

Before creating a branch, please create an
issue describing your intent.
This helps the community understand the
purpose of your work and allows for
discussion and feedback before you
begin implementation.

### Fork the Repository

Start by forking the repository to your
GitHub account.

### Clone the Repository

Clone the forked repository to your
local machine.

```shell
git clone https://github.com/ekgf/grapharg.git
cd grapharch
```

### Create a Branch

Once your issue has been approved, create a new branch for your feature or bug fix.

```shell
git switch -c feature/your-feature-name
```

### Make Your Changes

Implement your changes, make sure they are
well-tested, and follow the coding standards
of the project.

### Commit Your Changes

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

#### More detail

We enforce the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) standard.
Please use clear and descriptive commit messages that follow this convention.
Additionally, every commit should reference the issue it addresses in the message body or footer,
using `Closes #<issue-number>` or `Fixes #<issue-number>`.

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
git push origin feature/your-feature-name
```

### Create a Pull Request

Go to the original repository on GitHub and
open a pull request from your forked repository.
Describe your changes thoroughly in the pull
request description.

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

## Guidelines

### Coding Standards

Please ensure your code follows best practices
and is formatted correctly.
`rustfmt` must be used before committing,
and this will be verified by a
[GitHub Actions workflow](.github/workflows/check-formatting.yml),
just like the Conventional Commits check.

### Testing

Ensure that you write tests for new features
or bug fixes.
Use `cargo test` to run all tests and verify
that everything works as expected.

## Code of Conduct

We follow the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md).
By participating, you are expected to uphold this code.
Please report any behavior that violates this code to
[info@ekgf.org](mailto:info@ekgf.org).

## Issues and Feature Requests

If you encounter any issues, have questions
&mdash; or have feature requests &mdash; please open an issue on the
[GitHub issues page](https://github.com/ekgf/grapharg/issues).

## Contact

If you have any questions, feel free to reach out at [info@ekgf.org](mailto:info@ekgf.org).

---

Thank you for helping us make GraphArch a great tool for the community!
