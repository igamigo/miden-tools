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
distaff inspect <path> --validate --network devnet
```

| Flag | Description |
|------|-------------|
| `--validate` | Validate against a node. For notes: checks inclusion proof and nullifier status. For accounts: checks on-chain existence, state comparison, and staleness detection. |

#### Example: Inspect a Note File with Validation

```bash
# First, fetch and save a note from devnet
distaff rpc note 0x0e18ee4177e7c6b32d19e9a81200cb86a7afd50828a1c5384ffd2b8fc41e167e --network devnet --save note.mno

# Then inspect with validation
distaff inspect note.mno --validate --network devnet
```

Output:
```
Inspecting note.mno as NoteFile
- variant: NoteWithProof
- note id: 0x0e18ee4177e7c6b32d19e9a81200cb86a7afd50828a1c5384ffd2b8fc41e167e
- sender: 0x8b043136e8426720729a83b33f95a6
- type: Public
- tag: 0x3e800000
- assets: 1
- asset details:
  [0] fungible amount=1000000 faucet=0x8b043136e8426720729a83b33f95a6
- script root: 0xa657a127211172b9b305d06c6e076dd1edbf67c8b1a32c063647d5f7bf456131 (P2ID)
- created in block: 273675
- node index in block: 0
- inputs (P2ID):
  target account: 0xfa0000000000bb800000cc000000de

Validation (network: https://rpc.devnet.miden.io):
- validation path: local inclusion proof (block header check)
- local inclusion proof: ok (index 0)
- nullifier 0xa2d4a93f342f2d215ef16fb24e4696d06ac250abfb8c13e2e607523ca2188575 not found (unspent or not yet known)
```

### RPC Commands

Query Miden nodes directly:

```bash
distaff rpc status --network devnet
distaff rpc block <block-num> --network devnet
distaff rpc note <note-id> --network devnet
distaff rpc account <address-or-account-id> --verbose --network devnet
```

| Flag | Description |
|------|-------------|
| `--verbose` | Show detailed output (e.g., full account vault contents) |
| `--save <path>` | Save fetched note to a file (for `rpc note` command) |

#### Devnet Examples

```bash
# Check devnet status and latest block
distaff rpc status --network devnet

# Query the native asset faucet account
distaff rpc account 0xd0da1f806b49552007c49c95d519d7 --network devnet

# Get block details
distaff rpc block 273518 --network devnet

# Fetch a note by ID
distaff rpc note 0x0e18ee4177e7c6b32d19e9a81200cb86a7afd50828a1c5384ffd2b8fc41e167e --network devnet

# Fetch and save a note to a file
distaff rpc note 0x0e18ee4177e7c6b32d19e9a81200cb86a7afd50828a1c5384ffd2b8fc41e167e --network devnet --save note.mno
```

Example output for note query:
```
Note 0x0e18ee4177e7c6b32d19e9a81200cb86a7afd50828a1c5384ffd2b8fc41e167e:
- sender: 0x8b043136e8426720729a83b33f95a6
- type: Public
- tag: 0x3e800000
- included in block: 273675
- node index in block: 0
- assets: 1
- asset details:
  [0] fungible amount=1000000 faucet=0x8b043136e8426720729a83b33f95a6
- script root: 0xa657a127211172b9b305d06c6e076dd1edbf67c8b1a32c063647d5f7bf456131 (P2ID)
- inputs (P2ID):
  target account: 0xfa0000000000bb800000cc000000de
```

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
