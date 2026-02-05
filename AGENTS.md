# Repository Guidelines

## Project Structure & Module Organization
- Rust 2024 workspace; crates in `src/*`: `cmd` (CLI), `core` (engine), `cypher` (planning/opt), `storage` (persistence), shared utils in `common`, proc macros in `expr`.
- Test harness crates: `logictest` and `plannertest`; fixtures in `dataset/`; build outputs in `target/` (ignored).
- Tooling configs: `rustfmt.toml`, `taplo.toml`, and `rust-toolchain.toml` (nightly-2025-11-01 with `rustfmt`/`clippy`).

## Build, Test, and Development Commands
- Format: `make fmt` (`cargo fmt --all`); check-only: `make fmt-check`.
- Lint: `make clippy` (auto-fix, allows dirty) or CI-parity `make clippy-check`.
- Build: `make build` (debug) or `make build-release` (optimized workspace binaries).
- Test (preferred): `make test` → `cargo nextest run --no-fail-fast --all-targets --all-features --workspace`.
- Docs: `make docs_check` for CI; `make docs` to open locally.
- Run CLI: `cargo run -p cmd -- --db-path ./my_graph.elio` (omit path for in-memory).

## Coding Style & Naming Conventions
- Rustfmt enforced: 4 spaces, width 120, grouped imports (`StdExternalCrate`), reordered impl items. Run `make fmt` before PRs.
- TOML formatting via Taplo; run `taplo format` when editing `Cargo.toml` files.
- Naming: snake_case for modules/functions, UpperCamelCase for types; prefer concise domain-specific file names (`feat_xyz.rs`). Use `tracing` over `println!` for diagnostics; keep comments brief.

## Testing Guidelines
- Primary runner: `cargo nextest` via `make test`; keep tests deterministic.
- Unit tests colocated with code, named `test_<behavior>`.
- Behavior/plan coverage: use `src/logictest` and `src/plannertest`. Update insta baselines intentionally with `REWRITE=1 make rewrite-logic-test`.
- Add doc tests for user-facing examples and extend coverage when query semantics change.

## Commit & Pull Request Guidelines
- Commit style: `<type>: <summary>` (e.g., `feat`, `fix`, `chore`, `docs`, `test`, `refactor`), lowercase, ≤72 chars.
- Pre-PR checklist: `make fmt-check`, `make clippy-check`, `make test`. Note baseline rewrites or CLI outputs in the description; link issues.
- Keep PRs focused; avoid mixing refactors with new features. Rebase on `main` for a linear history.

## Configuration Tips
- Nightly toolchain pinned; `rust-toolchain.toml` removes need for `+nightly` suffix.
- RocksDB storage paths are local—use project-relative paths for reproducible runs; set `RUST_BACKTRACE=full` when debugging.
