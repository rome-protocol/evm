# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rome Protocol's fork of SputnikVM — a portable, stateless Ethereum Virtual Machine (EVM) written in Rust. The EVM is designed to be embedded into other systems (the caller provides state via a `Handler` trait).

## Build Commands

```bash
cargo build --release --all    # Build all crates
cargo test                     # `evm` package only — does NOT reach core/
cargo test --manifest-path core/Cargo.toml --features with-serde   # core/: needs the feature to link
cargo clippy                   # Lint (matches CI)
cargo fmt                      # Format
cargo fmt -- --check           # Check formatting without modifying
```

CI (`.github/workflows/ci.yml`) runs `cargo clippy` (lint job), then `cargo build`, `cargo test`, and `cargo test --manifest-path core/Cargo.toml --features with-serde` (build-and-test job, `needs: lint`) on push/PR to `master`. **The second `cargo test` invocation is load-bearing** — root `cargo test` covers the `evm` package only, and the core crate's tests need its `with-serde` feature to link, so without that manifest-path line every test under `core/tests/` is silently skipped rather than reported. `RUSTFLAGS: -Aunexpected_cfgs` is set at workflow level to tolerate pre-existing cfg warnings. A `concurrency` group cancels stale in-flight runs on the same ref.

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
- `eval/` subdirectory: opcode evaluation split into `arithmetic.rs`, `bitwise.rs`, `misc.rs`
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
- SELFDESTRUCT opcode disabled (PR #14). Reaches `Handler::other()` like any other undefined opcode.
- `Handler::other()` returns `Result<(), ExitFatal>` (PR #13, reaffirmed PR #37 / Halborn FIND-15). An undefined opcode or the disabled SELFDESTRUCT surfaces as `ExitReason::Fatal(ExitFatal::CallErrorAsFatal(ExitError::DesignatedInvalid))` — the caller cannot recover. A mid-Sept-2026 refactor (`a496349`) briefly narrowed this to a frame-local `ExitError` to match Ethereum's exceptional-halt semantics; PR #37 reverted the signature back to `ExitFatal` on the Halborn recommendation. Do not narrow it again without re-litigating that finding. Host implementations of the trait MUST return `ExitFatal`; `runtime/src/handler.rs` is the canonical signature.
- Halborn security audit fixes (PR #12, HAL-01 through HAL-12; PR #37 FIND-15).
- SPL-related modifications including `TransferProhibited` error variant (PR #9).
- Removed unused backend/gasometer crates (PR #11).
- Dropped unused `sha3` dep (PR #25) — SHA3 opcode goes through `Handler::keccak256_h256`.
- Per-opcode allocation overhead eliminated (PR #29) — `Stack::new` pre-reserves 64 slots, `Memory::new` pre-reserves 1 KiB, and `Memory::load_h256(offset)` reads a 32-byte word directly into an `H256` (no per-MLOAD `vec![0; 32]` heap alloc). Behaviour-preserving — `load_h256` is bit-identical to the prior `H256::from_slice(&get(offset, 32))` (verified by the differential test at `core/tests/load_h256.rs`); the two `with_capacity` hints are capacity-only and the Borsh-serialized form is byte-identical. Measured on a Uniswap V3 single-hop swap in the Solana SVM: −33,808 compute units (−2.54%), −23,552 peak heap (−14.5%), bit-identical output.
- `Config.memory_limit` bounded at 64 MiB (PR #31) — previously `usize::MAX`, which let a single MSTORE/CALLDATACOPY/MCOPY at a huge offset drive `Memory::set`'s `Vec::resize` unbounded and OOM the host in native off-chain emulation (the proxy's `catch_unwind` cannot contain an allocator abort). 64 MiB is far above any gas-affordable EVM memory; on-chain the SBF heap binds first, so behaviour is unchanged for real txs. A compile-time `const _: () = assert!(CONFIG.memory_limit > 0 && CONFIG.memory_limit <= 256 * 1024 * 1024)` in `tests/memory_limit.rs` fails the build if the default ever regresses to unbounded, plus a runtime `set_past_memory_limit_fails_closed` test asserts `ExitFatal::NotSupported` on writes past the limit. `Memory::resize_end` also enforces the limit before allocation (commit `6ceaf43`) so an oversized caller-chosen size is refused rather than attempted; zero-length `Memory::set` is a no-op regardless of offset (`c80678d`).

### Interpreter defect closures (Sept 2026, commits `4a4b58d` / `e5463a4` / `b1f51e9` / `608f16a` / `a496349` / PR #36)

A batch of fixes for attacker-chosen-operand and frame-local-error defects reachable from contract bytecode:

- **SDIV** (`4a4b58d`) — drop the sign-bit mask on the I256 quotient so `int256::MIN / 1` returns `int256::MIN`, not `0`. Verified against an independent yellow-paper model over 582,257 operand pairs.
- **BYTE / SIGNEXTEND** (`e5463a4`, `b1f51e9`) — bound the index in the U256 domain before narrowing to `usize`; SIGNEXTEND treats byte index 31 as the identity instead of building a 256-bit mask that underflows.
- **CALL dispatch** (`608f16a`) — validate `out_offset` / `out_len` and resize memory **before** dispatching the sub-call, so a bad range fails the caller's own frame instead of tripping a host assertion after the callee has already run. Guarded by `tests/call_dispatch.rs`, whose `CountingHandler::call_count` asserts `Handler::call` was never invoked on a rejected range.
- **JUMPI** (PR #36, `2c4b4ca`) — an untaken JUMPI whose destination exceeds `usize::MAX` continues, matching Ethereum; converting the destination to `usize` before checking the condition previously exited `InvalidJump` on otherwise-valid bytecode (GHSA-pvh2-pj76-4m96). The taken branch is unchanged — an unrepresentable or non-`JUMPDEST` destination is still `InvalidJump`. Test in `core/tests/oversized_operand.rs`.
- **Oversized copy operands** (`core/tests/oversized_operand.rs`, `tests/copy_offset_clamps.rs`) — an operand above `usize::MAX` used to construct `ExitFatal::NotSupported` (whole-tx death). Now: MSTORE's memory operand is a frame-local `ExitError::OutOfGas`; EXTCODECOPY / RETURNDATACOPY narrow via `as_usize_or_fail!` and behave to spec (EXTCODECOPY zero-fills, RETURNDATACOPY returns `OutOfOffset`).

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
- Integration tests live in TWO directories — `tests/` (uses the root `evm` package's re-exports) and `core/tests/` (drives `evm-core` directly and needs `--features with-serde` to link). Current suite (6 files):
  - `tests/memory_limit.rs` — PR #31, memory-limit fail-closed + compile-time `CONFIG.memory_limit` bound.
  - `tests/call_dispatch.rs` — CALL out-of-range operand rejected in the caller's frame before `Handler::call` is invoked (`CountingHandler::call_count == 0`).
  - `tests/copy_offset_clamps.rs` — EXTCODECOPY / RETURNDATACOPY behave to spec when their source offset exceeds `usize::MAX`.
  - `tests/undefined_opcode.rs` — asserts an undefined opcode / SELFDESTRUCT surfaces as `ExitReason::Fatal(CallErrorAsFatal(DesignatedInvalid))` (post-PR #37).
  - `core/tests/load_h256.rs` — PR #29, MLOAD differential vs. the pre-optimization `H256::from_slice(&get(offset, 32))`.
  - `core/tests/oversized_operand.rs` — frame-local behaviour of MSTORE / CODECOPY / CALLDATACOPY / JUMPI when a stack operand exceeds `usize::MAX`.

  Run everything the way CI does: `cargo test && cargo test --manifest-path core/Cargo.toml --features with-serde`. Broad opcode coverage still lives in `rome-evm-private/tests/`.

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
| Any opcode | `cargo test` **and** `cargo test --manifest-path core/Cargo.toml --features with-serde` here + `cd ../rome-evm-private && cargo test` + `tests/` opcode suite |
| Handler trait (esp. `Handler::other` signature) | Both test invocations + `tests/undefined_opcode.rs` + full rome-evm-private test suite |
| CALL / *COPY / JUMPI / arithmetic operand handling | `cargo test --test call_dispatch --test copy_offset_clamps` + `cargo test --manifest-path core/Cargo.toml --features with-serde --test oversized_operand` |
| Memory bounds / `Config.memory_limit` | `cargo test --test memory_limit` + downstream native emulator |
| Gas logic | `cargo test` + `tests/` opcode suite |
| Core (stack/memory, `load_h256`) | Both `cargo test` invocations + everything downstream |
