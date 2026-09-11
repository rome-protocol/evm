//! the finding, runtime-crate half: EXTCODECOPY and RETURNDATACOPY narrow
//! their source offset via `as_usize_or_fail!` before bounding it, so an
//! offset above `usize::MAX` hit the macro's Fatal path instead of the
//! spec's own behaviour (zero-fill for EXTCODECOPY, OutOfOffset for
//! RETURNDATACOPY — return data has a real length, unlike code, so an
//! out-of-range read is an explicit error rather than a zero-fill).
//! Run: `RUSTFLAGS=-Aunexpected_cfgs cargo test --test copy_offset_clamps`.

use evm::{
	Capture, Context, CreateScheme, ExitError, ExitFatal, ExitReason, ExitSucceed, H160, H256,
	Handler, Machine, Opcode, Runtime, Stack, Transfer, U256,
};

struct StubHandler {
	code: Vec<u8>,
}

impl Handler for StubHandler {
	type CreateInterrupt = ();
	type CreateFeedback = ();
	type CallInterrupt = ();
	type CallFeedback = ();

	fn keccak256_h256(&self, _data: &[u8]) -> H256 { H256::default() }
	fn nonce(&self, _address: H160) -> U256 { U256::zero() }
	fn balance(&self, _address: H160) -> U256 { U256::zero() }
	fn code_size(&self, _address: H160) -> U256 { U256::from(self.code.len()) }
	fn code_hash(&self, _address: H160) -> H256 { H256::default() }
	fn code(&self, _address: H160) -> Vec<u8> { self.code.clone() }
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
		unreachable!("not exercised")
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
		unreachable!("not exercised")
	}
	fn pre_validate(&mut self, _context: &Context, _opcode: Opcode, _stack: &Stack) -> Result<(), ExitError> { Ok(()) }
	fn other(&mut self, opcode: Opcode, _stack: &mut Machine) -> Result<(), ExitFatal> {
		// Not exercised by this suite's tests; mirrors the real handler's shape.
		let _ = opcode;
		Err(ExitFatal::CallErrorAsFatal(ExitError::DesignatedInvalid))
	}
}

fn push32(code: &mut Vec<u8>, val: U256) {
	code.push(0x7f); // PUSH32
	let mut buf = [0u8; 32];
	val.to_big_endian(&mut buf);
	code.extend_from_slice(&buf);
}

fn context() -> Context {
	Context { address: H160::zero(), caller: H160::zero(), apparent_value: U256::zero() }
}

#[test]
fn extcodecopy_with_source_offset_above_usize_max_zero_fills() {
	let mut code = Vec::new();
	push32(&mut code, U256::from(1)); // len
	push32(&mut code, U256::max_value()); // code_offset
	push32(&mut code, U256::zero()); // memory_offset
	push32(&mut code, U256::zero()); // address
	code.push(0x3c); // EXTCODECOPY
	push32(&mut code, U256::from(1)); // len
	push32(&mut code, U256::zero()); // start
	code.push(0xf3); // RETURN

	let mut handler = StubHandler { code: vec![0xaa; 4] };
	let mut runtime = Runtime::new(code, vec![0u8; 1], Vec::new(), context());
	match runtime.run(1000, &mut handler) {
		(_, Capture::Exit(reason)) => assert_eq!(reason, ExitReason::Succeed(ExitSucceed::Returned)),
		(_, Capture::Trap(_)) => panic!("expected Exit"),
	}
	assert_eq!(runtime.machine().return_value(), vec![0u8]);
}

#[test]
fn returndatacopy_with_offset_above_usize_max_is_out_of_offset_not_fatal() {
	let mut code = Vec::new();
	push32(&mut code, U256::from(1)); // len
	push32(&mut code, U256::max_value()); // data_offset
	push32(&mut code, U256::zero()); // memory_offset
	code.push(0x3e); // RETURNDATACOPY

	let mut handler = StubHandler { code: Vec::new() };
	let mut runtime = Runtime::new(code, vec![0u8; 1], Vec::new(), context());
	runtime.set_return_data(vec![0xaa; 4]);
	let (_, capture) = runtime.run(1000, &mut handler);

	match capture {
		Capture::Exit(reason) => assert_eq!(reason, ExitReason::Error(ExitError::OutOfOffset)),
		Capture::Trap(_) => panic!("expected Exit"),
	}
}
