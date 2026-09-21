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

CI (`.github/workflows/ci.yml`) runs `cargo clippy`, `cargo build`, `cargo test`, AND `cargo test --manifest-path core/Cargo.toml --features with-serde` on push/PR to `master`. **The core-crate test invocation is load-bearing** (added PR #34) — without it, the tests under `core/tests/` are silently skipped rather than failing, because a root `cargo test` doesn't reach them (the pre-PR-#34 CI ran 8 of 26 tests). `RUSTFLAGS: -Aunexpected_cfgs` is set at workflow level to tolerate pre-existing cfg warnings. A `concurrency` group cancels stale in-flight runs on the same ref.

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
- `Opcode`: all EVM opcode definitions (`opcode.rs`). `Opcode::SUICIDE = 0xff` is still defined but its dispatch arm in `runtime/src/eval/mod.rs` is commented out — SELFDESTRUCT is disabled at the interpreter level.
- `ExitReason`: execution termination types (Succeed, Error, Revert, Fatal, StepLimitReached)
- `eval/` subdirectory: opcode evaluation split into `arithmetic.rs`, `bitwise.rs`, `misc.rs`
- `memory.rs`: `Memory::resize_end` enforces `Config.memory_limit` before allocation (PR #34), refusing an oversized caller-chosen size with frame-local `ExitError::OutOfGas` rather than tripping the embedder's allocator. Zero-length `set()` is a no-op regardless of offset.
- `eval/macros.rs`: `as_usize_or_fail!` returns frame-local `ExitError::OutOfGas` (not `ExitFatal`) on an operand above `usize::MAX` — an unaffordable expansion is caller-recoverable, matching the gasometer-era behaviour.
- Primitive types (`H160`, `H256`, `U256`) defined via `fixed-hash` and `uint` macros in `primitive_types.rs`

**evm-runtime** (`runtime/src/`) — Execution interface connecting the machine to external state.
- `Runtime` struct: wraps `Machine` with execution context
- `Handler` trait (`handler.rs`): the key integration point — implementors provide account/block queries, storage mutations, and CALL/CREATE handling. `keccak256_h256` lives here (SHA3 opcode in `eval/system.rs` delegates to it; the EVM crate does not depend on `sha3`). **`Handler::other` returns `Result<(), ExitFatal>`** — an undefined opcode (or the disabled SELFDESTRUCT) surfaces as `ExitReason::Fatal`. PR #34 briefly flipped this to `ExitError` (frame-local); PR #37 reverted it back to `ExitFatal`. Downstream host implementations must match this signature.
- `Config` struct: hardfork feature flags (default: Istanbul). `Config.memory_limit` defaults to 64 MiB (PR #31).
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
- SELFDESTRUCT opcode disabled (PR #14).
- `Handler::other()` returns `ExitFatal` instead of `ExitError` (PR #13; PR #34 briefly flipped it to `ExitError`, PR #37 reverted).
- Halborn security audit fixes (PR #12, HAL-01 through HAL-12).
- SPL-related modifications including `TransferProhibited` error variant (PR #9).
- Removed unused backend/gasometer crates (PR #11).
- Dropped unused `sha3` dep (PR #25) — SHA3 opcode goes through `Handler::keccak256_h256`.
- Per-opcode allocation overhead eliminated (PR #29) — `Stack::new` pre-reserves 64 slots, `Memory::new` pre-reserves 1 KiB, and `Memory::load_h256(offset)` reads a 32-byte word directly into an `H256` (no per-MLOAD `vec![0; 32]` heap alloc). Behaviour-preserving — `load_h256` is bit-identical to the prior `H256::from_slice(&get(offset, 32))` (verified by the differential test at `core/tests/load_h256.rs`); the two `with_capacity` hints are capacity-only and the Borsh-serialized form is byte-identical. Measured on a Uniswap V3 single-hop swap in the Solana SVM: −33,808 compute units (−2.54%), −23,552 peak heap (−14.5%), bit-identical output.
- `Config.memory_limit` default lowered to 64 MiB (PR #31) — previously `usize::MAX`, which let a single MSTORE/CALLDATACOPY/MCOPY at a huge offset drive `Memory::set`'s `Vec::resize` unbounded and OOM the host in native off-chain emulation (the proxy's `catch_unwind` cannot contain an allocator abort). 64 MiB is far above any gas-affordable EVM memory; on-chain the SBF heap binds first, so behaviour is unchanged for real txs. **Enforcement moved into `Memory::resize_end` in PR #34** so an oversized caller-chosen size is refused before allocation (PR #31 shipped the default; PR #34 pushed the check down into core). A compile-time `const _: () = assert!(CONFIG.memory_limit <= 256 * 1024 * 1024)` in `tests/memory_limit.rs` fails the build if the default ever regresses.
- **Interpreter-defect closure over attacker-chosen operands** (PR #34, commit `e8075dc`):
  - **SDIV**: drops the sign-bit mask on the `I256` quotient so `int256::MIN / 1` returns `int256::MIN`, not `0`. Verified against an independent yellow-paper model over 582,257 operand pairs.
  - **BYTE / SIGNEXTEND**: bound the index in the U256 domain before narrowing to `usize`; SIGNEXTEND treats byte index 31 as the identity instead of building a 256-bit mask that underflows.
  - **CALL**: validates `out_offset` / `out_len` and resizes memory *before* dispatching the sub-call, so a bad range fails the caller's frame instead of tripping a host assertion after the callee has already run.
  - **Frame-local exits for oversized operands and copy paths**: undefined opcodes, the disabled SELFDESTRUCT, and oversized copy operands now fail only the frame that hit them (frame-local `ExitError`), matching Ethereum's exceptional-halt semantics — the callee's frame reverts and the caller receives 0 and continues. (The `Handler::other` return-type flip that PR #34 included was reverted by PR #37 — `Handler::other` itself is back to `ExitFatal`; the frame-local semantics for the interpreter-driven paths above stand.)
  - **EXTCODECOPY / RETURNDATACOPY**: source offset is bounded in the U256 domain before narrowing. EXTCODECOPY zero-fills on out-of-range (matches Ethereum); RETURNDATACOPY returns `OutOfOffset` because return data has a real length.
- **JUMPI operand handling** (PR #36, GHSA-pvh2-pj76-4m96): JUMPI now ignores the destination operand when the condition is zero. Previously converting the destination to `usize` before checking the condition made an untaken JUMPI whose destination exceeds `usize::MAX` exit `InvalidJump`, where Ethereum simply continues. The taken branch is unchanged — an unrepresentable or non-`JUMPDEST` destination is still `InvalidJump`.

## Dependency Management

- Dependabot (`.github/dependabot.yml`) runs weekly on Fridays for `cargo` and `github-actions` ecosystems.
- Auto-merge workflow (`.github/workflows/dependabot-auto-merge.yml`) merges patch + minor updates for `cargo`, `github_actions`, `npm_and_yarn`, `pip`, `gomod`. Major bumps and Docker stay manual.
- Solana-family crates (`solana-*`, `spl-token`, `anchor-*`, `curve25519-dalek`) are explicitly ignored — they are pinned via downstream consumers.
- `CHANGELOG.md` follows Keep a Changelog format; add entries under `[Unreleased]`.

## Agent Execution Guide

- This is a SputnikVM fork. Changes are rare and high-impact.
- Every opcode change affects all downstream repos (rome-evm-private, rome-sdk, rome-apps).
- After any change, run **both** `cargo test` **and** `cargo test --manifest-path core/Cargo.toml --features with-serde` — the root invocation alone silently skips every test under `core/tests/` (that's why CI runs both). Then verify `rome-evm-private` builds against the local checkout (`../evm` path dependency).
- The SELFDESTRUCT opcode is disabled — do not re-enable.
- `Handler::other` returns `Result<(), ExitFatal>`. Do not flip it back to `ExitError` without re-litigating the PR #34/#37 back-and-forth; downstream host implementations depend on this exact signature.
- Handler trait modifications affect all EVM execution paths.
- Six integration test files live here across the two crates. Broad opcode coverage still lives in `rome-evm-private/tests/`.
  - `tests/memory_limit.rs` — memory-limit fail-closed guard (PR #31) with compile-time invariant `const _: () = assert!(CONFIG.memory_limit <= 256 * 1024 * 1024)`.
  - `tests/undefined_opcode.rs` — asserts an undefined opcode / the disabled SELFDESTRUCT surfaces as `ExitReason::Fatal` (PR #34; the fatal outcome is what PR #37 restored via `Handler::other`).
  - `tests/call_dispatch.rs` — CALL validates `out_offset`/`out_len` and resizes memory BEFORE dispatching to `Handler::call`; a too-short CALL or out-of-range `out_offset` must fail the caller's frame without ever invoking the handler (PR #34).
  - `tests/copy_offset_clamps.rs` — EXTCODECOPY zero-fills on out-of-range source offset; RETURNDATACOPY returns `OutOfOffset` (PR #34).
  - `core/tests/load_h256.rs` — MLOAD differential (PR #29).
  - `core/tests/oversized_operand.rs` — MSTORE with an operand above `usize::MAX` is frame-local `OutOfGas` (not `ExitFatal`); CODECOPY/CALLDATACOPY clamp and zero-fill (PR #34); JUMPI on the untaken branch continues regardless of the destination (PR #36).

## Change Impact Map

| If you change... | Also check/update... |
|-----------------|---------------------|
| Any opcode implementation | `rome-evm-private/` (entrypoint macro, both program/ and emulator/) |
| Handler trait (esp. `other` return type) | `rome-evm-private/` (implements Handler); PR #34/#37 history |
| Gas calculations | `rome-evm-private/` gasometer, `tests/` opcode suite |
| `Memory::resize_end` / memory limit default | `tests/memory_limit.rs` compile-time invariant + `core/tests/oversized_operand.rs` |
| `as_usize_or_fail!` semantics | `core/tests/oversized_operand.rs` frame-local assertions |
| CI test invocation | Both `cargo test` AND `cargo test --manifest-path core/Cargo.toml --features with-serde` must stay wired in `ci.yml` (dropping the second silently skips `core/tests/`) |
| evm-core (stack, memory, eval) | All downstream: rome-evm-private, rome-sdk, rome-apps, tests |

## Test Selection Guide

| What Changed | Tests to Run |
|-------------|-------------|
| Any opcode | `cargo test` + `cargo test --manifest-path core/Cargo.toml --features with-serde` + `cd ../rome-evm-private && cargo test` + `tests/` opcode suite |
| Handler trait | Both cargo test invocations + full rome-evm-private test suite |
| Memory / gas logic | Both cargo test invocations + `tests/` opcode suite |
| Core (stack/memory/eval) | Both cargo test invocations + everything downstream |
| Attacker-operand paths (SDIV/BYTE/SIGNEXTEND/CALL out-range/copy offsets/JUMPI untaken) | `cargo test --manifest-path core/Cargo.toml --features with-serde --test oversized_operand` + `cargo test --test call_dispatch --test copy_offset_clamps --test undefined_opcode` |
