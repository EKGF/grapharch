# GraphArch

Generate documentation websites, specifications, and PDFs from a documentation
knowledge graph. The package installs the `grapharch` command.

After the first package release is published, choose an installation method:

```sh
uv tool install grapharch
```

Or install GraphArch in a project environment:

```sh
python -m pip install grapharch
grapharch --help
```

For an isolated one-off invocation:

```sh
uvx grapharch --help
```

Pin a GraphArch version in your dependency configuration for reproducible CI.
Packages contain the executable; running GraphArch does not require downloading
an executable from a separate website.

Prebuilt packages are prepared for Linux with glibc 2.28+ and Windows on
x64 and ARM64, and for macOS 11+ on Apple silicon. Intel Macs are not supported. See the
[documentation](https://ekgf.github.io/grapharch/) for source configuration,
publication commands, and examples.
