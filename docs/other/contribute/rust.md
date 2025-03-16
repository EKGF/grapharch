---
title: "Rust Notes"
layout: default
parent: Contribute
---

# Rust Notes

## Installation

See [Rust installation](./rust-install.md).

## Updating

Regularly update your Rust toolchain with the following command which
takes care of everything:

```shell
rustup update
```

## GraphArch Rust Documentation

To generate the technical documentation of GraphArch, from the source code,
run the following command:

```shell
cargo doc --open
```

## Upgrade Dependencies

The `Cargo.toml` file in the root of the [repository](https://github.com/EKGF/grapharch/blob/main/Cargo.toml) specifies all the dependent rust packages (called "crates") and their version numbers.

As a developer, in your clone of the repo, you can upgrade these dependencies to their
latest versions by using the following command:

```shell
cargo upgrade
```