# Distaff

Lightweight CLI utilities for inspecting Miden files, local stores, and RPC endpoints.

## Usage

```bash
distaff file <path-to-note-or-account-file>
distaff file <path> --validate --network testnet
distaff rpc status
distaff rpc note <note-id> --network testnet
distaff rpc account <address-or-account-id> --network devnet --verbose
distaff store inspect --store <path-to-sqlite>
distaff store tx inspect <tx-id> --store <path-to-sqlite> --verbose
distaff store tx list --store <path-to-sqlite>
distaff store account --store <path-to-sqlite> --account <address-or-id>
distaff word <felt1> <felt2> <felt3> <felt4>
```

Networks:
- `testnet` (default)
- `devnet`
- `local`
- `custom` (requires `--endpoint protocol://host[:port]`)

Completions:
```bash
distaff completions zsh
```

## Development

- `make format` – format the workspace
- `make clippy` – lint with warnings denied
- `make test` – run tests
- `make install` – install the CLI locally

## Notes

- Built against `miden-client` `0.12.5`.
- Licensed under MIT (see `LICENSE`).
- Reference coding guidelines: https://github.com/0xMiden/miden-client/blob/next/CONTRIBUTING.md.
