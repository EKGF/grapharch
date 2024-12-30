---
title: "MacOS Notes"
layout: default
permalink: /contribute/macos
---

# MacOS Notes

In order to contribute please install a developer environment

1. [Install rust](#install-rust-with-brew)
2. Install utilities
   - [cocogitto](#cocogitto)
   - [yamlfmt](#yamlfmt)
3. Install an IDE
   - [Visual Studio Code](https://code.visualstudio.com/docs/setup/mac)
   - [RustRover](https://www.jetbrains.com/rust/)
   - Or any other IDE you prefer.
4. [Setup Git](git-setup.md)

## yamlfmt

Install yamlfmt &mdash; to format `.yml` or `.yaml` files &mdash;
by using `brew`:

```shell
brew install yamlfmt
```

## cocogitto

We recommend using cocogitto for anything to do with "conventional commits".

Install it as follows:

```shell
cargo install cocogitto
cog install-hook --all
```

Check the [Cocogitto User Guide](https://docs.cocogitto.io/guide/init.html)
