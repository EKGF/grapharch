# MacOS notes

## install rust with brew

On the Rust website, they recommend to [install rust](https://www.rust-lang.org/tools/install)
using this command:

```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

As you can see, it first installs `rustup` which is kind of a special install script
for all the various tools around rust, including rust itself.
They call that a "tool chain".
With `rustup` you can install or update or change your rust toolchain.

Rustup itself does not really change much, so you can just as well install it
via homebrew instead and leave it to a regular 'brew upgrade' to upgrade rustup
now and then as well.

```shell
brew install rustup
```

Then go the git repo root directory and type

```shell
rustup toolchain install nightly
rustup override set nightly
```

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
