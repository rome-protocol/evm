//! FIND-015(a): `Handler::other()` returns `Result<(), ExitFatal>`, so an
//! implementor has no way to signal "this callee failed" the way every other
//! trait method (frame-local `ExitError`) can. The trait's only implementor
//! (rome-evm-private program/src/state/handler.rs) therefore reports
//! undefined opcodes and the disabled SELFDESTRUCT as `ExitFatal`, which
//! kills the whole transaction instead of just the frame that hit them.
//!
//! This stub targets the FIXED signature (`Result<(), ExitError>`) directly:
//! it will not compile against the current trait, which is the RED for a
//! signature change (a compile-time failure is the honest RED here, not a
//! runtime one — there is no way to make the old signature produce the new
//! behaviour). Run: `RUSTFLAGS=-Aunexpected_cfgs cargo test --test undefined_opcode`.

use evm::{
	Capture, Context, CreateScheme, ExitError, ExitReason, H160, H256, Handler, Machine, Opcode,
	Runtime, Stack, Transfer, U256,
};

struct StubHandler;

impl Handler for StubHandler {
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

	// The fixed shape: mirrors rome-evm-private's post-fix handler — log and
	// return the frame-local DesignatedInvalid, not a whole-tx Fatal.
	fn other(&mut self, _opcode: Opcode, _stack: &mut Machine) -> Result<(), ExitError> {
		Err(ExitError::DesignatedInvalid)
	}
}

fn context() -> Context {
	Context { address: H160::zero(), caller: H160::zero(), apparent_value: U256::zero() }
}

#[test]
fn undefined_opcode_is_frame_local_designated_invalid() {
	let mut handler = StubHandler;
	let mut runtime = Runtime::new(vec![0x0c], vec![0u8; 1], Vec::new(), context());
	let (_, capture) = runtime.run(10, &mut handler);

	match capture {
		Capture::Exit(reason) => assert_eq!(reason, ExitReason::Error(ExitError::DesignatedInvalid)),
		Capture::Trap(_) => panic!("expected Exit"),
	}
}

#[test]
fn disabled_selfdestruct_is_frame_local_designated_invalid() {
	let mut handler = StubHandler;
	let mut runtime = Runtime::new(vec![0xff], vec![0u8; 1], Vec::new(), context());
	let (_, capture) = runtime.run(10, &mut handler);

	match capture {
		Capture::Exit(reason) => assert_eq!(reason, ExitReason::Error(ExitError::DesignatedInvalid)),
		Capture::Trap(_) => panic!("expected Exit"),
	}
}
