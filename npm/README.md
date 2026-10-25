# GraphArch

Generate documentation websites, specifications, and PDFs from a documentation
knowledge graph.

After the first package release is published:

```sh
npm install --save-dev grapharch
npx grapharch --help
```

Pin GraphArch in your project's lockfile and invoke it in your normal npm scripts
or CI pipeline. Install optional dependencies: they contain the executable for
your operating system. Installation and invocation do not download an executable
from a separate website.

Packages are prepared for Linux with glibc 2.28+, macOS 11+, and Windows, on
Intel/AMD 64-bit and ARM64. Unsupported platforms and missing or mismatched
platform packages fail with an explicit error.

See the [documentation](https://ekgf.github.io/grapharch/) for source configuration,
publication commands, and examples.
