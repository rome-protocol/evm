# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rome Protocol's fork of SputnikVM — a portable, stateless Ethereum Virtual Machine (EVM) written in Rust. The EVM is designed to be embedded into other systems (the caller provides state via a `Handler` trait).

## Build Commands

```bash
cargo build                    # Build the root crate (pulls in evm-core + evm-runtime via path deps)
cargo test                     # Run all tests (returns 0 from root — see "Agent Execution Guide" below)
cargo clippy                   # Lint (matches CI)
cargo fmt                      # Format
cargo fmt -- --check           # Check formatting without modifying
```

`Cargo.toml` has **no `[workspace]` table** — `evm-core` and `evm-runtime` are pulled in as path dependencies from the root crate, not as workspace members. As a result, `cargo build --all` / `cargo test --all` from the root only operate on the root crate; sub-crate dev-deps + unit tests don't run via the workspace. `cargo test -p evm-core` errors out (`cannot be tested because it requires dev-dependencies and is not a member of the workspace`).

CI (`.github/workflows/ci.yml`) runs `cargo clippy`, `cargo build`, and `cargo test` on push/PR to `master`. `RUSTFLAGS: -Aunexpected_cfgs` is set at workflow level to tolerate pre-existing cfg warnings from the `fixed-hash` / `uint` macros (which emit unknown cfgs like `dev` that the crate-level `#![deny(warnings)]` would otherwise reject). A `concurrency` group cancels stale in-flight runs on the same ref. The `build-and-test` job uses `Swatinem/rust-cache@v2` and depends on `lint`.

## Lint Configuration

The root `src/lib.rs` enforces strict linting at compile time:
- `#![deny(warnings)]`
- `#![forbid(unsafe_code, missing_docs, unused_variables, unused_imports)]`
- `#![deny(clippy::all, clippy::pedantic, clippy::nursery)]` (with `module_name_repetitions`, `missing_errors_doc`, `missing_panics_doc` allowed)

All new public items require doc comments. Unsafe code is forbidden. Note: CI clippy runs without `-D warnings` — the crate-level `deny(warnings)` inside `lib.rs` (and `evm-core` source) is what surfaces lint failures during build. This is why `RUSTFLAGS=-Aunexpected_cfgs` must be set for `cargo build` too, not just clippy.

## Architecture

Three crates in a layered architecture (the upstream `evm-gasometer` crate has been removed — see PR #11):

**evm** (root, `src/lib.rs`) — Top-level re-export crate. Re-exports everything from `evm-core` and `evm-runtime`.

**evm-core** (`core/src/`) — Low-level bytecode interpreter. No blockchain state awareness.
- `Machine` struct: stack, memory, program counter, bytecode execution
- `Opcode`: all EVM opcode definitions (`opcode.rs`)
- `ExitReason`: execution termination types (Succeed, Error, Revert, Fatal, StepLimitReached)
- `eval/` subdirectory: opcode evaluation split into `arithmetic.rs`, `bitwise.rs`, `misc.rs` (plus `macros.rs` for shared dispatch macros and `mod.rs` for the eval entry point)
- Primitive types (`H160`, `H256`, `U256`) defined via `fixed-hash` and `uint` macros in `primitive_types.rs`

**evm-runtime** (`runtime/src/`) — Execution interface connecting the machine to external state.
- `Runtime` struct: wraps `Machine` with execution context
- `Handler` trait (`handler.rs`): the key integration point — implementors provide account/block queries, storage mutations, and CALL/CREATE handling. `keccak256_h256` lives here (SHA3 opcode in `eval/system.rs` delegates to it; the EVM crate does not depend on `sha3`).
- `Config` struct: hardfork feature flags (default: Istanbul)
- Interrupt model (`interrupt.rs`): `Capture<E, T>` enum with `Exit` (done) or `Trap` (needs external input for CALL/CREATE)

### Execution Flow

1. Caller creates `Runtime` with bytecode, context, and `Config`
2. Runtime steps through opcodes via `Machine`
3. On CALL/CREATE, runtime returns `Capture::Trap(Resolve::Call/Create(...))`
4. Caller resolves the interrupt (executes sub-call, updates state)
5. Feeds result back into the `Resolve` handle; execution resumes

## Feature Flags

- `std` (default): standard library support
- `with-serde`: serde serialization
- `with-codec`: parity-scale-codec support
- All crates support `no_std` (with `alloc`)
- `borsh` serialization is always enabled for Machine/Runtime state checkpointing (not gated by a feature flag — `borsh` is a non-optional dep in both `core/Cargo.toml` and `runtime/Cargo.toml`)

## Rome Protocol Modifications

Key changes from upstream SputnikVM:
- SELFDESTRUCT opcode disabled (PR #14)
- `Handler::other()` returns `ExitFatal` instead of `ExitError` (PR #13)
- Halborn security audit fixes (PR #12, HAL-01 through HAL-12)
- SPL-related modifications including `TransferProhibited` error variant (PR #9)
- Removed unused backend/gasometer crates (PR #11)
- Dropped unused `sha3` dep (PR #25) — SHA3 opcode goes through `Handler::keccak256_h256`

## Dependency Management

- Dependabot (`.github/dependabot.yml`) runs weekly on Fridays for `cargo` and `github-actions` ecosystems.
- Auto-merge workflow (`.github/workflows/dependabot-auto-merge.yml`) merges patch + minor updates for `cargo`, `github_actions`, `npm_and_yarn`, `pip`, `gomod`. Major bumps and Docker stay manual.
- Solana-family crates (`solana-*`, `spl-token`, `anchor-*`, `curve25519-dalek`) are explicitly ignored — they are pinned via downstream consumers.
- `CHANGELOG.md` follows Keep a Changelog format; add entries under `[Unreleased]`.

## Agent Execution Guide

- This is a SputnikVM fork. Changes are rare and high-impact.
- Every opcode change affects all downstream repos (rome-evm-private, rome-sdk, rome-apps).
- After any change, run `cargo test` here, then verify `rome-evm-private` builds against the local checkout (`../evm` path dependency).
- The SELFDESTRUCT opcode is disabled — do not re-enable.
- Handler trait modifications affect all EVM execution paths.
- `cargo test` from the root returns 0 tests. A single inline unit test exists in `core/src/valids.rs` but doesn't run because `evm-core` is a path dep, not a workspace member. Real opcode + integration coverage lives in `rome-evm-private/tests/` and the `tests/` integration repo.

## Change Impact Map

| If you change... | Also check/update... |
|-----------------|---------------------|
| Any opcode implementation | `rome-evm-private/` (entrypoint macro, both program/ and emulator/) |
| Handler trait | `rome-evm-private/` (implements Handler) |
| Gas calculations | `rome-evm-private/` gasometer, `tests/` opcode suite |
| evm-core (stack, memory) | All downstream: rome-evm-private, rome-sdk, rome-apps, tests |

## Test Selection Guide

| What Changed | Tests to Run |
|-------------|-------------|
| Any opcode | `cargo test` here + `cd ../rome-evm-private && cargo test` + `tests/` opcode suite |
| Handler trait | `cargo test` + full rome-evm-private test suite |
| Gas logic | `cargo test` + `tests/` opcode suite |
| Core (stack/memory) | `cargo test` + everything downstream |
