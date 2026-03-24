# LiquiFact Contracts

Soroban smart contracts for **LiquiFact** — the global invoice liquidity network on Stellar. This repo contains the **escrow** contract that holds investor funds for tokenized invoices until settlement.

Part of the LiquiFact stack: **frontend** (Next.js) | **backend** (Express) | **contracts** (this repo).

---

## Prerequisites

- **Rust** 1.70+ (stable)
- **Soroban CLI** (optional, for deployment): [Stellar Soroban docs](https://developers.stellar.org/docs/smart-contracts/getting-started/soroban-cli)

For CI and local checks you only need Rust and `cargo`.

---

## Setup

1. **Clone the repo**

   ```bash
   git clone <this-repo-url>
   cd liquifact-contracts
   ```

2. **Build**

   ```bash
   cargo build
   ```

3. **Run tests**

   ```bash
   cargo test
   ```

---

## Development

| Command           | Description                    |
|-------------------|--------------------------------|
| `cargo build`     | Build all contracts            |
| `cargo test`      | Run unit tests                 |
| `cargo fmt`       | Format code                    |
| `cargo fmt -- --check` | Check formatting (used in CI) |

---

## Project structure

```
liquifact-contracts/
├── Cargo.toml           # Workspace definition
├── escrow/
│   ├── Cargo.toml       # Escrow contract crate
│   └── src/
│       ├── lib.rs       # LiquiFact escrow contract (init, fund, settle)
│       └── test.rs      # Unit tests
└── .github/workflows/
    └── ci.yml           # CI: fmt, build, test
```

### Escrow contract (high level)

- **init** — Create an invoice escrow (admin, invoice id, SME address, amount, yield bps, maturity). Requires `admin` authorization.
- **get_escrow** — Read current escrow state (no auth required).
- **fund** — Record investor funding; status becomes “funded” when target is met. Requires `investor` authorization.
- **settle** — Mark escrow as settled (buyer paid; investors receive principal + yield). Requires `sme_address` authorization.

### Authorization model

All sensitive state transitions are protected by Soroban's native [`require_auth`](https://developers.stellar.org/docs/smart-contracts/example-contracts/auth) mechanism.

| Function | Required Signer  | Rationale                                                  |
|----------|------------------|------------------------------------------------------------|
| `init`   | `admin`          | Prevents unauthorized escrow creation or re-initialization |
| `fund`   | `investor`       | Each investor authorizes their own contribution            |
| `settle` | `sme_address`    | Only the SME beneficiary may trigger settlement            |

`require_auth` integrates with Soroban's authorization framework: on-chain, the transaction must carry a valid signature (or sub-invocation auth) from the required address. In tests, `env.mock_all_auths()` satisfies all checks so happy-path logic can be verified independently of key management.

#### Security assumptions

- The `admin` address is trusted to create legitimate escrows. Rotate or use a multisig address in production.
- Re-initialization is blocked at the contract level (`"Escrow already initialized"` panic) regardless of who calls `init`.
- `settle` can only move status from `1 → 2`; calling it on an open or already-settled escrow panics.

---

## CI/CD

GitHub Actions runs on every push and pull request to `main`:

- **Format** — `cargo fmt --all -- --check`
- **Build** — `cargo build`
- **Tests** — `cargo test`

Keep formatting and tests passing before opening a PR.

---

## Contributing

1. **Fork** the repo and clone your fork.
2. **Create a branch** from `main`: `git checkout -b feature/your-feature` or `fix/your-fix`.
3. **Setup**: ensure Rust stable is installed; run `cargo build` and `cargo test`.
4. **Make changes**:
   - Follow existing patterns in `escrow/src/lib.rs`.
   - Add or update tests in `escrow/src/test.rs`.
   - Format with `cargo fmt`.
5. **Verify locally**:
   - `cargo fmt --all -- --check`
   - `cargo build`
   - `cargo test`
6. **Commit** with clear messages (e.g. `feat(escrow): X`, `test(escrow): Y`).
7. **Push** to your fork and open a **Pull Request** to `main`.
8. Wait for CI and address review feedback.

We welcome new contracts (e.g. settlement, tokenization helpers), tests, and docs that align with LiquiFact’s invoice financing flow.

---

## License

MIT (see root LiquiFact project for full license).
