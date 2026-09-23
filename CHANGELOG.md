# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Added
- `Machine::new_shared`, `Runtime::new_shared`, `Valids::shared`: a machine's code and its jump-destination map are held as `Rc<Vec<u8>>`, so a caller can copy a contract's code out of its account once per execution and hand every frame of that address the same copy (rome-evm-canton, state shape step 6: the T-REX transfer ran one authority's 9 KB seven times). `Machine::new`, `Runtime::new` and `Valids::new` keep their signatures and wrap. The Borsh and serde wire is unchanged: a length and the bytes (`core/src/rc_bytes.rs`; `core/tests/shared_code.rs`).

### Fixed
- `JUMPI` with a zero condition now continues regardless of the destination operand, as on Ethereum; previously a destination above `usize::MAX` on an untaken branch exited `InvalidJump` (GHSA-pvh2-pj76-4m96). The taken branch is unchanged: an unrepresentable or non-`JUMPDEST` destination is still `InvalidJump`.

### Added
- Agent Execution Guide and Change Impact Map in CLAUDE.md
- PR and issue templates for standardized contributions
- CI pipeline with lint and test stages
