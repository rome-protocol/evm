//! Differential test for `Memory::load_h256` — the no-allocation MLOAD path.
//!
//! `load_h256(offset)` must be BIT-IDENTICAL to the prior MLOAD implementation
//! `H256::from_slice(&memory.get(offset, 32)[..])` for every input, since MLOAD
//! is consensus-critical. This exercises the interpreter `Memory` directly (no
//! contract / no swap) across every boundary: empty memory, reads past the
//! written region (zero-pad), partial words at the tail, exact-fit, and the
//! `offset + 32 > limit` guard. Run: `RUSTFLAGS=-Aunexpected_cfgs cargo test`.

use evm_core::{Memory, H256};

/// The exact expression MLOAD used before `load_h256` existed.
fn reference(mem: &Memory, offset: usize) -> H256 {
    H256::from_slice(&mem.get(offset, 32)[..])
}

fn assert_equiv(mem: &Memory, offset: usize) {
    assert_eq!(
        mem.load_h256(offset),
        reference(mem, offset),
        "load_h256 diverged from get()+from_slice at offset {offset}"
    );
}

#[test]
fn load_h256_matches_reference_across_boundaries() {
    let limit = 1024 * 1024;
    let mut mem = Memory::new(limit);

    // Write a recognizable, non-zero pattern over [0, 200): byte i = i+1.
    let pattern: Vec<u8> = (0u16..200).map(|i| (i % 251 + 1) as u8).collect();
    mem.set(0, &pattern, Some(pattern.len())).unwrap();

    // Cover: word-aligned, mid-word, the written/zero boundary, partial tail,
    // fully past data, and large offsets near the limit.
    let offsets = [
        0, 1, 2, 31, 32, 33, 63, 64, 95,
        168, 169, 170, 199, 200, 201,   // 200 = data.len(): tail / past-end edge
        255, 256, 1000, 4095, 4096,
        limit - 32, limit - 1, limit, limit + 1,
    ];
    for off in offsets {
        assert_equiv(&mem, off);
    }
}

#[test]
fn load_h256_on_empty_memory_is_zero() {
    let mem = Memory::new(1024 * 1024);
    assert_eq!(mem.load_h256(0), H256::default());
    assert_eq!(mem.load_h256(0), reference(&mem, 0));
    assert_eq!(mem.load_h256(64), reference(&mem, 64));
}

#[test]
fn load_h256_partial_tail_word_is_zero_padded() {
    // Memory written to exactly 40 bytes; reading a 32-byte word at offset 16
    // straddles the written/unwritten boundary (16..40 written, 40..48 zero).
    let mut mem = Memory::new(1024 * 1024);
    let data: Vec<u8> = (0u8..40).map(|i| i + 1).collect();
    mem.set(0, &data, Some(40)).unwrap();
    for off in 0..=48 {
        assert_equiv(&mem, off);
    }
}

#[test]
fn load_h256_respects_limit_guard() {
    // Small limit: any read whose window crosses `limit` must return zeros,
    // identically to `get`.
    let limit = 96usize;
    let mut mem = Memory::new(limit);
    mem.set(0, &[0xABu8; 64], Some(64)).unwrap();
    for off in [0, 32, 63, 64, 65, 96, 97, usize::MAX - 16] {
        assert_equiv(&mem, off);
    }
}
