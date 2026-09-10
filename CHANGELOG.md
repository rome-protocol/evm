# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Fixed
- `JUMPI` with a zero condition now continues regardless of the destination operand, as on Ethereum; previously a destination above `usize::MAX` on an untaken branch exited `InvalidJump` (GHSA-pvh2-pj76-4m96). The taken branch is unchanged: an unrepresentable or non-`JUMPDEST` destination is still `InvalidJump`.

### Added
- Agent Execution Guide and Change Impact Map in CLAUDE.md
- PR and issue templates for standardized contributions
- CI pipeline with lint and test stages
