# miden-tools

A collection of command-line utilities for working with the [Miden](https://github.com/0xPolygonMiden) ecosystem.

## Tools

| Tool | Description |
|------|-------------|
| [**teasel**](teasel/README.md) | Lightweight CLI utilities for inspecting Miden files, local `miden-client` stores, and RPC endpoints. |
| [**snowberry**](snowberry/README.md) | CLI tool for inspecting Miden MASP package files across multiple `miden-mast-package` versions (0.13–0.22). |

See each tool's README for detailed usage.

## Installation

```bash
git clone https://github.com/igamigo/miden-tools.git
cd miden-tools

make install-teasel      # Install the teasel CLI
make install-snowberry   # Install the snowberry CLI
```

Alternatively, build the whole workspace:

```bash
cargo build --release
```

## Development

This repository is a Cargo workspace. Common tasks are available through the `Makefile`:

```bash
make help            # Show all commands
make lint            # Run format, clippy, taplo, and typos checks
make format          # Format Rust sources (nightly rustfmt)
make clippy          # Lint with warnings denied
make test            # Run tests
```

## Contributing

For contributing, see the [miden-client contributing guide](https://github.com/0xPolygonMiden/miden-client/blob/main/CONTRIBUTING.md), which also applies here.

## License

Licensed under the MIT License (see [`LICENSE`](LICENSE)).
