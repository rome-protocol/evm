# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rome Protocol's fork of SputnikVM — a portable, stateless Ethereum Virtual Machine (EVM) written in Rust. The EVM is designed to be embedded into other systems (the caller provides state via a `Handler` trait).

## Build Commands

```bash
cargo build --release --all    # Build all crates
cargo test                     # Run all tests
cargo clippy                   # Lint (matches CI)
cargo fmt                      # Format
cargo fmt -- --check           # Check formatting without modifying
```

CI (`.github/workflows/ci.yml`) runs `cargo clippy` (without `-D warnings`), `cargo build`, and `cargo test` on push/PR to `master`. `RUSTFLAGS: -Aunexpected_cfgs` is set at workflow level to tolerate pre-existing cfg warnings.

## Lint Configuration

The root `src/lib.rs` enforces strict linting at compile time:
- `#![deny(warnings)]`
- `#![forbid(unsafe_code, missing_docs, unused_variables, unused_imports)]`
- `#![deny(clippy::all, clippy::pedantic, clippy::nursery)]` (with `module_name_repetitions`, `missing_errors_doc`, `missing_panics_doc` allowed)

All new public items require doc comments. Unsafe code is forbidden. Note: CI clippy runs without `-D warnings` — the crate-level `deny(warnings)` inside `lib.rs` is what surfaces lint failures during build.

## Architecture

Three crates in a layered architecture:

**evm** (root) — Top-level re-export crate. Re-exports everything from `evm-core` and `evm-runtime`.

**evm-core** (`core/`) — Low-level bytecode interpreter. No blockchain state awareness.
- `Machine` struct: stack, memory, program counter, bytecode execution
- `Opcode`: all EVM opcode definitions
- `ExitReason`: execution termination types (Succeed, Error, Revert, Fatal, StepLimitReached)
- `eval/` subdirectory: opcode evaluation split into `arithmetic.rs`, `bitwise.rs`, `misc.rs`
- Primitive types (`H160`, `H256`, `U256`) defined via `fixed-hash` and `uint` macros in `primitive_types.rs`

**evm-runtime** (`runtime/`) — Execution interface connecting the machine to external state.
- `Runtime` struct: wraps `Machine` with execution context
- `Handler` trait: the key integration point — implementors provide account/block queries, storage mutations, and CALL/CREATE handling
- `Config` struct: hardfork feature flags (default: Istanbul)
- Interrupt model: `Capture<E, T>` enum with `Exit` (done) or `Trap` (needs external input for CALL/CREATE)

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
- `borsh` serialization is always enabled for Machine/Runtime state checkpointing

## Rome Protocol Modifications

Key changes from upstream SputnikVM (visible in recent PRs):
- SELFDESTRUCT opcode disabled (PR #14)
- `Handler::other()` returns `ExitFatal` instead of `ExitError` (PR #13)
- Halborn security audit fixes (PR #12)
- SPL-related modifications including `TransferProhibited` error variant (PR #9)
- Removed unused backend/gasometer crates (PR #11)

## Agent Execution Guide

- This is a SputnikVM fork. Changes are rare and high-impact.
- Every opcode change affects all downstream repos (rome-evm-private, rome-sdk, rome-apps).
- After any change, run `cargo test` here, then verify `rome-evm-private` builds against the local checkout (`../evm` path dependency).
- The SELFDESTRUCT opcode is disabled — do not re-enable.
- Handler trait modifications affect all EVM execution paths.

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
