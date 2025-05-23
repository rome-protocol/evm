use core::cmp::min;
use alloc::vec::Vec;
use crate::{Runtime, ExitError, Handler, Capture, Transfer, ExitReason, CreateScheme, CallScheme, Context, ExitSucceed, ExitFatal, H160, H256, U256};
use super::Control;

/// Compute Keccak-256 hash
pub fn sha3<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	pop_u256!(runtime, from, len);
	let from = as_usize_or_fail!(from);
	let len = as_usize_or_fail!(len);

	try_or_fail!(runtime.machine.memory_mut().resize_offset(from, len));
	let data = if len == 0 {
		Vec::new()
	} else {
		runtime.machine.memory_mut().get(from, len)
	};

	let ret = handler.keccak256_h256(data.as_slice()); //Keccak256::digest(data.as_slice());
	push!(runtime, ret); //H256::from_slice(ret.as_slice()));

	Control::Continue
}


// Get the chain ID
pub fn chainid<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push_u256!(runtime, handler.chain_id());

	Control::Continue
}

/// Get address of currently executing account
pub fn address<H: Handler>(runtime: &mut Runtime) -> Control<H> {
	let ret = H256::from(runtime.context.address);
	push!(runtime, ret);

	Control::Continue
}

/// Get balance of the given account
pub fn balance<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	pop!(runtime, address);
	push_u256!(runtime, handler.balance(address.into()));

	Control::Continue
}

/// Get balance of currently executing account
pub fn selfbalance<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push_u256!(runtime, handler.balance(runtime.context.address));

	Control::Continue
}

/// Get the base fee
pub fn basefee<H: Handler>(runtime: &mut Runtime, _handler: &H) -> Control<H> {
	push_u256!(runtime, U256::zero());

	Control::Continue
}

/// Get execution origination address
pub fn origin<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	let ret = H256::from(handler.origin());
	push!(runtime, ret);

	Control::Continue
}

/// Get caller address
pub fn caller<H: Handler>(runtime: &mut Runtime) -> Control<H> {
	let ret = H256::from(runtime.context.caller);
	push!(runtime, ret);

	Control::Continue
}

/// Get deposited value by the instruction/transaction responsible for this execution
pub fn callvalue<H: Handler>(runtime: &mut Runtime) -> Control<H> {
	let mut ret = H256::default();
	runtime.context.apparent_value.to_big_endian(&mut ret[..]);
	push!(runtime, ret);

	Control::Continue
}

/// Get price of gas in current environment
pub fn gasprice<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	let mut ret = H256::default();
	handler.gas_price().to_big_endian(&mut ret[..]);
	push!(runtime, ret);

	Control::Continue
}

/// Get size of an account’s code
pub fn extcodesize<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	pop!(runtime, address);
	push_u256!(runtime, handler.code_size(address.into()));

	Control::Continue
}

/// Get hash of an account’s code
pub fn extcodehash<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	pop!(runtime, address);
	push!(runtime, handler.code_hash(address.into()));

	Control::Continue
}

/// Copy an account’s code to memory
pub fn extcodecopy<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	pop!(runtime, address);
	pop_u256!(runtime, memory_offset, code_offset, len);

	let memory_offset = as_usize_or_fail!(memory_offset);
	let code_offset = as_usize_or_fail!(code_offset);
	let len = as_usize_or_fail!(len);

	try_or_fail!(runtime.machine.memory_mut().resize_offset(memory_offset, len));
	match runtime.machine.memory_mut().copy_large(
		memory_offset,
		code_offset,
		len,
		&handler.code(address.into())
	) {
		Ok(()) => (),
		Err(e) => return Control::Exit(e.into()),
	};

	Control::Continue
}

/// Get size of output data from the previous call from the current environment
pub fn returndatasize<H: Handler>(runtime: &mut Runtime) -> Control<H> {
	let size = U256::from(runtime.return_data_buffer.len());
	push_u256!(runtime, size);

	Control::Continue
}

/// Copy output data from the previous call to memory
pub fn returndatacopy<H: Handler>(runtime: &mut Runtime) -> Control<H> {
	pop_u256!(runtime, memory_offset, data_offset, len);

	let memory_offset = as_usize_or_fail!(memory_offset);
	let data_offset = as_usize_or_fail!(data_offset);
	let len = as_usize_or_fail!(len);

	try_or_fail!(runtime.machine.memory_mut().resize_offset(memory_offset, len));
	if data_offset.checked_add(len)
		.map(|l| l > runtime.return_data_buffer.len())
		.unwrap_or(true)
	{
		return Control::Exit(ExitError::OutOfOffset.into())
	}

	match runtime.machine.memory_mut().copy_large(memory_offset, data_offset, len, &runtime.return_data_buffer) {
		Ok(()) => Control::Continue,
		Err(e) => Control::Exit(e.into()),
	}
}

/// Get the hash of one of the 256 most recent complete blocks
pub fn blockhash<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	pop_u256!(runtime, number);
	push!(runtime, handler.block_hash(number));

	Control::Continue
}

/// Get the block’s beneficiary address
pub fn coinbase<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push!(runtime, handler.block_coinbase().into());
	Control::Continue
}

/// Get the block’s timestamp
pub fn timestamp<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push_u256!(runtime, handler.block_timestamp());
	Control::Continue
}

/// Get the block’s number
pub fn number<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push_u256!(runtime, handler.block_number());
	Control::Continue
}

/// Get the block’s difficulty (PREVRANDAO)
pub fn difficulty<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push_u256!(runtime, handler.block_difficulty());
	Control::Continue
}

/// Get the block’s gas limit
pub fn gaslimit<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push_u256!(runtime, handler.block_gas_limit());
	Control::Continue
}

/// Load word from storage
pub fn sload<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	pop_u256!(runtime, index);
	let value = handler.storage(runtime.context.address, index);
	push_u256!(runtime, value);

	Control::Continue
}

/// Save word to storage
pub fn sstore<H: Handler>(runtime: &mut Runtime, handler: &mut H) -> Control<H> {
	pop_u256!(runtime, index, value);

	match handler.set_storage(runtime.context.address, index, value) {
		Ok(()) => Control::Continue,
		Err(e) => Control::Exit(e.into()),
	}
}

/// Load word from transient storage
pub fn tload<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	pop_u256!(runtime, index);
	let value = handler.transient_storage(runtime.context.address, index);
	push_u256!(runtime, value);

	Control::Continue
}

/// Save word to transient storage
pub fn tstore<H: Handler>(runtime: &mut Runtime, handler: &mut H) -> Control<H> {
	pop_u256!(runtime, index, value);

	match handler.set_transient_storage(runtime.context.address, index, value) {
		Ok(()) => Control::Continue,
		Err(e) => Control::Exit(e.into()),
	}
}

/// Get the amount of available gas, including the corresponding reduction for the cost of
/// this instruction
pub fn gas<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push_u256!(runtime, handler.gas_left());

	Control::Continue
}


/// Append log record
pub fn log<H: Handler>(runtime: &mut Runtime, n: u8, handler: &mut H) -> Control<H> {
	pop_u256!(runtime, offset, len);
	let offset = as_usize_or_fail!(offset);
	let len = as_usize_or_fail!(len);

	try_or_fail!(runtime.machine.memory_mut().resize_offset(offset, len));
	let data = if len == 0 {
		Vec::new()
	} else {
		runtime.machine.memory().get(offset, len)
	};

	let mut topics = Vec::new();
	for _ in 0..(n as usize) {
		match runtime.machine.stack_mut().pop() {
			Ok(value) => { topics.push(value); }
			Err(e) => return Control::Exit(e.into()),
		}
	}

	match handler.log(runtime.context.address, topics, data) {
		Ok(()) => Control::Continue,
		Err(e) => Control::Exit(e.into()),
	}
}

// Halt execution and register account for later deletion or send all Ether to address (post-Cancun)
pub fn suicide<H: Handler>(runtime: &mut Runtime, handler: &mut H) -> Control<H> {
	pop!(runtime, target);

	match handler.mark_delete(runtime.context.address, target.into()) {
		Ok(()) => (),
		Err(e) => return Control::Exit(e.into()),
	}

	Control::Exit(ExitSucceed::Suicided.into())
}

/// Create a new account with associated code
pub fn create<H: Handler>(
	runtime: &mut Runtime,
	is_create2: bool,
	handler: &mut H,
) -> Control<H> {
	runtime.return_data_buffer = Vec::new();

	pop_u256!(runtime, value, code_offset, len);
	let code_offset = as_usize_or_fail!(code_offset);
	let len = as_usize_or_fail!(len);

	try_or_fail!(runtime.machine.memory_mut().resize_offset(code_offset, len));
	let code = if len == 0 {
		Vec::new()
	} else {
		runtime.machine.memory().get(code_offset, len)
	};

	let scheme = if is_create2 {
		pop!(runtime, salt);
		//let code_hash = H256::from_slice(Keccak256_digest(&code)); //Keccak256::digest(&code).as_slice());
		let code_hash = handler.keccak256_h256(&code);
		CreateScheme::Create2 {
			caller: runtime.context.address,
			salt,
			code_hash,
		}
	} else {
		CreateScheme::Legacy {
			caller: runtime.context.address,
		}
	};

	match handler.create(runtime.context.address, scheme, value, code, None) {
		Capture::Exit((reason, address, _return_data)) => {
			save_created_address(runtime, reason, address)
		},
		Capture::Trap(interrupt) => {
			// The created contract's address will be push by the method save_created_address()
			// push!(runtime, H256::default());
			Control::CreateInterrupt(interrupt)
		},
	}
}

/// Message-call into an account
pub fn call<'config, H: Handler>(
	runtime: &mut Runtime,
	scheme: CallScheme,
	handler: &mut H,
) -> Control<H> {
	runtime.return_data_buffer = Vec::new();

	pop_u256!(runtime, gas);
	pop!(runtime, to);
	let gas = if gas > U256::from(u64::MAX) {
		None
	} else {
		Some(gas.as_u64())
	};

	let value = match scheme {
		CallScheme::Call | CallScheme::CallCode => {
			pop_u256!(runtime, value);
			value
		},
		CallScheme::DelegateCall | CallScheme::StaticCall => {
			U256::zero()
		},
	};

	// out_offset and out_len parameters will be read in save_return_value()
	pop_u256!(runtime, in_offset, in_len/*, out_offset, out_len*/);
	let in_offset = as_usize_or_fail!(in_offset);
	let in_len = as_usize_or_fail!(in_len);
	
	try_or_fail!(runtime.machine.memory_mut().resize_offset(in_offset, in_len));
	// try_or_fail!(runtime.machine.memory_mut().resize_offset(out_offset, out_len));

	let input = if in_len == 0 {
		Vec::new()
	} else {
		runtime.machine.memory().get(in_offset, in_len)
	};

	let context = match scheme {
		CallScheme::Call | CallScheme::StaticCall => Context {
			address: to.into(),
			caller: runtime.context.address,
			apparent_value: value,
		},
		CallScheme::CallCode => Context {
			address: runtime.context.address,
			caller: runtime.context.address,
			apparent_value: value,
		},
		CallScheme::DelegateCall => Context {
			address: runtime.context.address,
			caller: runtime.context.caller,
			apparent_value: runtime.context.apparent_value,
		},
	};

	let transfer = if scheme == CallScheme::Call {
		Some(Transfer {
			source: runtime.context.address,
			target: to.into(),
			value: value.into()
		})
	} else if scheme == CallScheme::CallCode {
		Some(Transfer {
			source: runtime.context.address,
			target: runtime.context.address,
			value: value.into()
		})
	} else {
		None
	};

	match handler.call(to.into(), transfer, input, gas, scheme == CallScheme::StaticCall, context) {
		Capture::Exit((reason, return_data)) => {
			save_return_value(runtime, reason, return_data)
		},
		Capture::Trap(interrupt) => {
			// The result of the call opcode will be push by the method save_return_value()
			// push!(runtime, H256::default());
			Control::CallInterrupt(interrupt)
		},
	}
}

/// save created contract address into parent runtime
pub fn save_created_address<'config, H: Handler>(
	runtime: &mut Runtime,
	reason : ExitReason,
	address: Option<H160>,
) -> Control<H> {
	// runtime.return_data_buffer = return_data;
	let create_address: H256 = address.map(|a| a.into()).unwrap_or_default();

	match reason {
		ExitReason::Succeed(_) => {
			push!(runtime, create_address.into());
			Control::Continue
		},
		ExitReason::Revert(_) => {
			push!(runtime, H256::default());
			Control::Continue
		},
		ExitReason::Error(_) => {
			push!(runtime, H256::default());
			Control::Continue
		},
		ExitReason::Fatal(e) => {
			push!(runtime, H256::default());
			Control::Exit(e.into())
		},
		ExitReason::StepLimitReached => { unreachable!() }
	}

}

/// save return_value into parent runtime
pub fn save_return_value<'config, H: Handler>(
	runtime: &mut Runtime,
	reason : ExitReason,
	return_data : Vec<u8>,
	) -> Control<H> {

	pop_u256!(runtime, out_offset, out_len);
	let out_offset = as_usize_or_fail!(out_offset);
	let out_len = as_usize_or_fail!(out_len);

	try_or_fail!(runtime.machine.memory_mut().resize_offset(out_offset, out_len));

        {  // this block uses the given alignment to match the original code.
			runtime.return_data_buffer = return_data;
			let target_len = min(out_len, runtime.return_data_buffer.len());

			match reason {
				ExitReason::Succeed(_) => {
					match runtime.machine.memory_mut().copy_large(
						out_offset,
						0,
						target_len,
						&runtime.return_data_buffer[..],
					) {
						Ok(()) => {
							push_u256!(runtime, U256::one());
							Control::Continue
						},
						Err(_) => {
							push_u256!(runtime, U256::zero());
							Control::Continue
						},
					}
				},
				ExitReason::Revert(_) => {
					push_u256!(runtime, U256::zero());

					let _ = runtime.machine.memory_mut().copy_large(
						out_offset,
						0,
						target_len,
						&runtime.return_data_buffer[..],
					);

					Control::Continue
				},
				ExitReason::Error(_) => {
					push_u256!(runtime, U256::zero());

					Control::Continue
				},
				ExitReason::Fatal(e) => {
					push_u256!(runtime, U256::zero());

					Control::Exit(e.into())
				},
				ExitReason::StepLimitReached => { unreachable!() }
			}
        }
}
