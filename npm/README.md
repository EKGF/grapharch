# GraphArch

Generate documentation websites, specifications, and PDFs from a documentation
knowledge graph.

After the first package release is published:

```sh
npm install --save-dev @ekgf/grapharch
npx grapharch --help
```

Pin GraphArch in your project's lockfile and invoke it in your normal npm scripts
or CI pipeline. Install optional dependencies: they contain the executable for
your operating system. Installation and invocation do not download an executable
from a separate website.

Packages are prepared for Linux with glibc 2.28+ and Windows on x64 and
ARM64, and for macOS 11+ on Apple silicon. Intel Macs are not supported. Unsupported platforms and missing or mismatched
platform packages fail with an explicit error.

See the [documentation](https://ekgf.github.io/grapharch/) for source configuration,
publication commands, and examples.
