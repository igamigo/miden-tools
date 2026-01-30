# Distaff

Lightweight CLI utilities for inspecting Miden files, local stores, and RPC endpoints.

## Usage

```bash
# Inspect files
distaff inspect <path-to-note-or-account-file>
distaff inspect <path> --validate --network testnet

# RPC commands
distaff rpc status --network testnet
distaff rpc block <block-num> --network testnet
distaff rpc note <note-id> --network testnet
distaff rpc account <address-or-account-id> --network devnet --verbose

# Store commands
distaff store path
distaff store info --store <path-to-sqlite>
distaff store account list --store <path-to-sqlite>
distaff store account get --account <address-or-id> --store <path-to-sqlite>
distaff store note list --store <path-to-sqlite>
distaff store note get <note-id> --store <path-to-sqlite>
distaff store tag list --store <path-to-sqlite>
distaff store tx list --store <path-to-sqlite>
distaff store tx inspect <tx-id> --store <path-to-sqlite> --verbose
distaff store tui --store <path-to-sqlite>

# Parsing helpers
distaff parse word <felt1> <felt2> <felt3> <felt4>
distaff parse account-id <address-or-account-id> --network testnet
distaff parse address <bech32-or-account-id> --network testnet
distaff parse note-tag <tag>
```

Networks:
- `testnet` (default)
- `devnet`
- `local`
- `custom` (requires `--endpoint protocol://host[:port]`)

Completions:
```bash
```

## Development

- `make format` – format the workspace
- `make clippy` – lint with warnings denied
- `make test` – run tests
- `make install` – install the CLI locally

## Notes

- Built against `miden-client` `0.13`.
- Licensed under MIT (see `LICENSE`).
- Reference coding guidelines: https://github.com/0xMiden/miden-client/blob/next/CONTRIBUTING.md.
