# Distaff

Lightweight CLI utilities for inspecting Miden files, local stores, and RPC endpoints.

## Installation

```bash
# From source
git clone https://github.com/youruser/miden-tools.git
cd miden-tools
make install
```

## Configuration

Distaff automatically reads `miden-client` configuration if present:
- `.miden/miden-client.toml` (local directory)
- `~/.miden/miden-client.toml` (global)

When a config is found, store path and RPC endpoint are used as defaults. You can override them with `--store` and `--network`/`--endpoint` flags.

## Usage

### Inspect Files

Inspect serialized note or account files:

```bash
distaff inspect <path-to-note-or-account-file>
distaff inspect <path> --validate --network testnet
```

| Flag | Description |
|------|-------------|
| `--validate` | Validate the note against a node (checks inclusion proof and nullifier status) |

### RPC Commands

Query Miden nodes directly:

```bash
distaff rpc status --network testnet
distaff rpc block <block-num> --network testnet
distaff rpc note <note-id> --network testnet
distaff rpc account <address-or-account-id> --verbose
```

| Flag | Description |
|------|-------------|
| `--verbose` | Show detailed output (e.g., full account vault contents) |
| `--save <path>` | Save fetched note to a file (for `rpc note` command) |

### Store Commands

Inspect local miden-client sqlite stores:

```bash
distaff store path                                    # Print default store path
distaff store info                                    # Print store summary
distaff store account list                            # List tracked accounts
distaff store account get --account <address-or-id>   # Get account details
distaff store note list                               # List notes
distaff store note get <note-id>                      # Get note details
distaff store tag list                                # List tracked note tags
distaff store tx list                                 # List transactions
distaff store tx inspect <tx-id> --verbose            # Inspect transaction
distaff store tui                                     # Interactive store browser
```

| Flag | Description |
|------|-------------|
| `--store <path>` | Use a custom store path instead of the default |
| `--verbose` | Show detailed transaction info (for `tx inspect`) |

### Parsing Helpers

Parse and convert common Miden formats:

```bash
distaff parse word <felt1> <felt2> <felt3> <felt4>    # Build word from felts
distaff parse account-id <address-or-id>              # Parse account ID
distaff parse address <bech32-or-id> --network testnet
distaff parse note-tag <tag>                          # Parse note tag
```

### Networks

- `testnet` (default)
- `devnet`
- `local`
- `custom` (requires `--endpoint protocol://host[:port]`)

## Development

```bash
make help       # Show all commands
make format     # Format with rustfmt (nightly)
make clippy     # Lint with warnings denied
make test       # Run tests
make install    # Install locally
```

## Contributing

For contributing to the underlying `miden-client` library, see the [miden-client contributing guide](https://github.com/0xPolygonMiden/miden-client/blob/main/CONTRIBUTING.md).

## Notes

- Built against `miden-client` `0.13`.
- Licensed under MIT (see `LICENSE`).
