---
title: "MacOS Notes"
layout: default
parent: Contribute
---

# MacOS Notes

In order to contribute please install a developer environment

1. [Install Homebrew](https://brew.sh/)&mdash;i.e. the `brew` command, used to install other tools
1. [Install rust](#install-rust-with-brew)&mdash;the primary programming language used to create GraphArch
1. Install utilities
   - [cocogitto](#cocogitto)&mdash;a tool, the `cog` command, for "conventional commits"
   - [yamlfmt](#yamlfmt)&mdash;to format `.yml` or `.yaml` files
1. Install an IDE
   - [Visual Studio Code](https://code.visualstudio.com/docs/setup/mac)
   - [Cursor AI](https://www.cursor.com/)&mdash;an AI-powered IDE, our favorite
   - [RustRover](https://www.jetbrains.com/rust/)
   - Or any other IDE you prefer.
1. [Setup Git](git-setup.md)

## Install HomeBrew

Paste the following into your terminal:

```shell
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

The `brew` command is used to install most other tools.

## Install yamlfmt

Install yamlfmt &mdash; to format `.yml` or `.yaml` files &mdash;
by using `brew`:

```shell
brew install yamlfmt
```

## Install cocogitto

We recommend using cocogitto for anything to do with ["conventional commits"](./contribute-code.md#conventional-commits).

Install it as follows, assuming you have `cargo` installed (if not, see [Install Rust](./rust-install.md)):

```shell
cargo install cocogitto
cog install-hook --all
```

Check the [Cocogitto User Guide](https://docs.cocogitto.io/guide/init.html)
