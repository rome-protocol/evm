//! DoS guard: the interpreter's `Config.memory_limit` bounds the backing
//! `Vec::resize` in `Memory::set`. With `usize::MAX` (the prior default) a
//! single MSTORE/CALLDATACOPY/MCOPY at a huge offset resizes unbounded → host
//! OOM in native (off-chain) emulation, which the proxy's `catch_unwind` cannot
//! contain (an allocator abort is not an unwind). The default must be finite.
//! Run: `RUSTFLAGS=-Aunexpected_cfgs cargo test`.

use evm::{Memory, ExitFatal, CONFIG};

// Compile-time invariant: the emulation memory limit is finite and sane — never
// `usize::MAX`. A regression to an unbounded default fails to compile here.
const _: () = assert!(
    CONFIG.memory_limit > 0 && CONFIG.memory_limit <= 256 * 1024 * 1024,
    "CONFIG.memory_limit must be finite and bounded (<= 256 MiB)",
);

#[test]
fn set_past_memory_limit_fails_closed() {
    // With the limit armed (bounded by the const assert above, so `+ 32` cannot
    // overflow), a write past it returns NotSupported rather than attempting an
    // unbounded allocation. `Memory::new` pre-reserves 1 KiB and the guard
    // rejects before `resize`, so this allocates nothing.
    let mut mem = Memory::new(CONFIG.memory_limit);
    let past = CONFIG.memory_limit + 32;
    assert_eq!(
        mem.set(past, &[0u8; 32], Some(32)),
        Err(ExitFatal::NotSupported),
        "set() past memory_limit must fail closed",
    );
}
