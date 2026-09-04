//! FIND-015(b)/(c): an operand above `usize::MAX` used to construct
//! `ExitFatal::NotSupported` (whole-transaction death downstream). A memory
//! operand (MSTORE) should instead be a frame-local `ExitError::OutOfGas`;
//! an unpriced *COPY source offset (CODECOPY/CALLDATACOPY) should clamp and
//! zero-fill, matching Ethereum, rather than fail at all.
//! Run: `RUSTFLAGS=-Aunexpected_cfgs cargo test --features with-serde --test oversized_operand`.

use evm_core::{Capture, Context, ExitError, ExitReason, ExitSucceed, H160, Machine, U256, Valids};

const STACK_LIMIT: usize = 1024;
const MEMORY_LIMIT: usize = 64 * 1024 * 1024;

fn push32(code: &mut Vec<u8>, val: U256) {
	code.push(0x7f); // PUSH32
	let mut buf = [0u8; 32];
	val.to_big_endian(&mut buf);
	code.extend_from_slice(&buf);
}

fn zero_context() -> Context {
	Context { address: H160::zero(), caller: H160::zero(), apparent_value: U256::zero() }
}

#[test]
fn mstore_with_index_above_usize_max_is_frame_local_out_of_gas() {
	let mut code = Vec::new();
	push32(&mut code, U256::zero()); // value
	push32(&mut code, U256::max_value()); // index
	code.push(0x52); // MSTORE

	let valids = Valids::compute(&code);
	let mut machine = Machine::new(code, valids, Vec::new(), STACK_LIMIT, MEMORY_LIMIT);
	let ok = |_, _: &_| Ok(());
	let (_, capture) = machine.run(1000, ok, &zero_context());

	assert_eq!(capture, Capture::Exit(ExitReason::Error(ExitError::OutOfGas)));
}

#[test]
fn codecopy_with_source_offset_above_usize_max_zero_fills() {
	let mut code = Vec::new();
	push32(&mut code, U256::from(1)); // len
	push32(&mut code, U256::max_value()); // code_offset
	push32(&mut code, U256::zero()); // memory_offset
	code.push(0x39); // CODECOPY
	push32(&mut code, U256::from(1)); // len
	push32(&mut code, U256::zero()); // start
	code.push(0xf3); // RETURN

	let valids = Valids::compute(&code);
	let mut machine = Machine::new(code, valids, Vec::new(), STACK_LIMIT, MEMORY_LIMIT);
	let ok = |_, _: &_| Ok(());
	let (_, capture) = machine.run(1000, ok, &zero_context());

	assert_eq!(capture, Capture::Exit(ExitReason::Succeed(ExitSucceed::Returned)));
	assert_eq!(machine.return_value(), vec![0u8]);
}

#[test]
fn calldatacopy_with_source_offset_above_usize_max_zero_fills() {
	let mut code = Vec::new();
	push32(&mut code, U256::from(1)); // len
	push32(&mut code, U256::max_value()); // data_offset
	push32(&mut code, U256::zero()); // memory_offset
	code.push(0x37); // CALLDATACOPY
	push32(&mut code, U256::from(1)); // len
	push32(&mut code, U256::zero()); // start
	code.push(0xf3); // RETURN

	let valids = Valids::compute(&code);
	let data = vec![0xaa_u8; 4];
	let mut machine = Machine::new(code, valids, data, STACK_LIMIT, MEMORY_LIMIT);
	let ok = |_, _: &_| Ok(());
	let (_, capture) = machine.run(1000, ok, &zero_context());

	assert_eq!(capture, Capture::Exit(ExitReason::Succeed(ExitSucceed::Returned)));
	assert_eq!(machine.return_value(), vec![0u8]);
}
