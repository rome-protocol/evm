//! FIND-013 / FIND-010(3) / FIND-015(d): `call()` must validate the CALL's
//! stack operands (including out_offset/out_len) *before* dispatching to
//! `Handler::call`, not after the callee has already run. A too-short CALL
//! (missing out_offset/out_len) or an out-of-range out_offset must fail the
//! caller's own frame without ever invoking the handler.
//! Run: `RUSTFLAGS=-Aunexpected_cfgs cargo test --test call_dispatch`.

use evm::{
	Capture, Context, CreateScheme, ExitError, ExitReason, ExitSucceed,
	H160, H256, Handler, Machine, Opcode, Runtime, Stack, Transfer, U256, CONFIG,
};

/// Records whether `Handler::call` was ever invoked. Every other method is a
/// trivial stub; this suite never reaches them.
struct CountingHandler {
	call_count: u32,
}

impl Handler for CountingHandler {
	type CreateInterrupt = ();
	type CreateFeedback = ();
	type CallInterrupt = ();
	type CallFeedback = ();

	fn keccak256_h256(&self, _data: &[u8]) -> H256 { H256::default() }
	fn nonce(&self, _address: H160) -> U256 { U256::zero() }
	fn balance(&self, _address: H160) -> U256 { U256::zero() }
	fn code_size(&self, _address: H160) -> U256 { U256::zero() }
	fn code_hash(&self, _address: H160) -> H256 { H256::default() }
	fn code(&self, _address: H160) -> Vec<u8> { Vec::new() }
	fn valids(&self, _address: H160) -> Vec<u8> { Vec::new() }
	fn storage(&self, _address: H160, _index: U256) -> U256 { U256::zero() }
	fn transient_storage(&self, _address: H160, _index: U256) -> U256 { U256::zero() }
	fn gas_left(&self) -> U256 { U256::zero() }
	fn gas_price(&self) -> U256 { U256::zero() }
	fn origin(&self) -> H160 { H160::default() }
	fn block_hash(&self, _number: U256) -> H256 { H256::default() }
	fn block_number(&self) -> U256 { U256::zero() }
	fn block_coinbase(&self) -> H160 { H160::default() }
	fn block_timestamp(&self) -> U256 { U256::zero() }
	fn block_difficulty(&self) -> U256 { U256::zero() }
	fn block_gas_limit(&self) -> U256 { U256::zero() }
	fn chain_id(&self) -> U256 { U256::zero() }
	fn set_storage(&mut self, _address: H160, _index: U256, _value: U256) -> Result<(), ExitError> { Ok(()) }
	fn set_transient_storage(&mut self, _address: H160, _index: U256, _value: U256) -> Result<(), ExitError> { Ok(()) }
	fn log(&mut self, _address: H160, _topics: Vec<H256>, _data: Vec<u8>) -> Result<(), ExitError> { Ok(()) }
	fn mark_delete(&mut self, _address: H160, _target: H160) -> Result<(), ExitError> { Ok(()) }
	fn create(
		&mut self,
		_caller: H160,
		_scheme: CreateScheme,
		_value: U256,
		_init_code: Vec<u8>,
		_target_gas: Option<u64>,
	) -> Capture<(ExitReason, Option<H160>, Vec<u8>), Self::CreateInterrupt> {
		Capture::Exit((ExitSucceed::Stopped.into(), None, Vec::new()))
	}
	fn call(
		&mut self,
		_code_address: H160,
		_transfer: Option<Transfer>,
		_input: Vec<u8>,
		_target_gas: Option<u64>,
		_is_static: bool,
		_context: Context,
	) -> Capture<(ExitReason, Vec<u8>), Self::CallInterrupt> {
		self.call_count += 1;
		Capture::Exit((ExitSucceed::Stopped.into(), Vec::new()))
	}
	fn pre_validate(&mut self, _context: &Context, _opcode: Opcode, _stack: &Stack) -> Result<(), ExitError> { Ok(()) }
	fn other(&mut self, opcode: Opcode, _stack: &mut Machine) -> Result<(), ExitError> {
		// Not exercised by this suite's tests; mirrors the fixed real handler's
		// frame-local shape.
		let _ = opcode;
		Err(ExitError::DesignatedInvalid)
	}
}

fn push32(code: &mut Vec<u8>, val: U256) {
	code.push(0x7f); // PUSH32
	let mut buf = [0u8; 32];
	val.to_big_endian(&mut buf);
	code.extend_from_slice(&buf);
}

fn context() -> Context {
	Context {
		address: H160::repeat_byte(1),
		caller: H160::repeat_byte(2),
		apparent_value: U256::zero(),
	}
}

/// FIND-013: only 5 of the 7 CALL operands are on the stack (out_offset and
/// out_len were never pushed). The callee must not run before the missing
/// operands are discovered.
#[test]
fn call_with_too_few_operands_does_not_dispatch() {
	let mut code = Vec::new();
	push32(&mut code, U256::zero()); // in_len
	push32(&mut code, U256::zero()); // in_offset
	push32(&mut code, U256::zero()); // value
	push32(&mut code, U256::from(3)); // to
	push32(&mut code, U256::zero()); // gas
	code.push(0xf1); // CALL

	let mut handler = CountingHandler { call_count: 0 };
	let mut runtime = Runtime::new(code, vec![0u8; 1], Vec::new(), context());
	let (_, capture) = runtime.run(1000, &mut handler);

	assert_eq!(handler.call_count, 0, "a too-short CALL must not dispatch the sub-call");
	match capture {
		Capture::Exit(reason) => assert_eq!(reason, ExitReason::Error(ExitError::StackUnderflow)),
		Capture::Trap(_) => panic!("expected Exit, got a Trap (interrupt)"),
	}
}

/// FIND-010(3) / FIND-015(d): a full 7-operand CALL whose out_offset is past
/// the memory limit must fail the caller's own frame *before* the callee
/// runs (today it dispatches, then fails only when save_return_value reads
/// out_offset/out_len after the callee has already returned).
#[test]
fn call_with_out_of_range_out_offset_does_not_dispatch() {
	let mut code = Vec::new();
	push32(&mut code, U256::from(32)); // out_len
	push32(&mut code, U256::from(1u64 << 36)); // out_offset: far past the 64 MiB limit
	push32(&mut code, U256::zero()); // in_len
	push32(&mut code, U256::zero()); // in_offset
	push32(&mut code, U256::zero()); // value
	push32(&mut code, U256::from(3)); // to
	push32(&mut code, U256::zero()); // gas
	code.push(0xf1); // CALL

	let mut handler = CountingHandler { call_count: 0 };
	let mut runtime = Runtime::new(code, vec![0u8; 1], Vec::new(), context());
	let (_, capture) = runtime.run(1000, &mut handler);

	assert_eq!(
		handler.call_count, 0,
		"an out-of-range out_offset must fail the caller's frame before the callee runs"
	);
	match capture {
		Capture::Exit(reason) => assert_eq!(reason, ExitReason::Error(ExitError::OutOfGas)),
		Capture::Trap(_) => panic!("expected Exit, got a Trap (interrupt)"),
	}
}

/// Control: a full, in-range CALL still dispatches exactly once and succeeds.
#[test]
fn call_with_valid_operands_dispatches_once() {
	let mut code = Vec::new();
	push32(&mut code, U256::zero()); // out_len
	push32(&mut code, U256::zero()); // out_offset
	push32(&mut code, U256::zero()); // in_len
	push32(&mut code, U256::zero()); // in_offset
	push32(&mut code, U256::zero()); // value
	push32(&mut code, U256::from(3)); // to
	push32(&mut code, U256::from(CONFIG.stack_limit as u64)); // gas (arbitrary, in range)
	code.push(0xf1); // CALL
	code.push(0x00); // STOP

	let mut handler = CountingHandler { call_count: 0 };
	let mut runtime = Runtime::new(code, vec![0u8; 1], Vec::new(), context());
	let (_, capture) = runtime.run(1000, &mut handler);

	assert_eq!(handler.call_count, 1);
	assert!(matches!(capture, Capture::Exit(ExitReason::Succeed(ExitSucceed::Stopped))));
}
