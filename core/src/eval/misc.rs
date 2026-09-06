use core::cmp::{min, max};
use super::Control;
use crate::{Machine, ExitError, ExitSucceed, ExitRevert, H256, U256};

/// Get size of code running in current environment
pub fn codesize(state: &mut Machine) -> Control {
	let size = U256::from(state.code.len());
	push_u256!(state, size);
	Control::Continue(1)
}

/// Copy code running in current environment to memory
pub fn codecopy(state: &mut Machine) -> Control {
	pop_u256!(state, memory_offset, code_offset, len);

	let memory_offset = as_usize_or_fail!(memory_offset);
	// Clamp before narrowing, not after: an out-of-range source offset must
	// zero-fill (spec), not fail converting to usize.
	let code_offset = min(code_offset, U256::from(state.code.len())).as_usize();
	let len = as_usize_or_fail!(len);

	try_or_fail!(state.memory.resize_offset(memory_offset, len));
	match state.memory.copy_large(memory_offset, code_offset, len, &state.code) {
		Ok(()) => Control::Continue(1),
		Err(e) => Control::Exit(e.into()),
	}
}

/// Get input data of current environment
pub fn calldataload(state: &mut Machine) -> Control {
	pop_u256!(state, index);

	let index = as_usize_or_fail!(index);
	let mut load = [0_u8; 32];

	if index < state.data.len() {
		let len = min(32, state.data.len() - index);
		load[0..len].copy_from_slice(&state.data[index..index + len]);
	}

	push!(state, H256::from(load));
	Control::Continue(1)
}

/// Get size of input data in current environment
pub fn calldatasize(state: &mut Machine) -> Control {
	let len = U256::from(state.data.len());
	push_u256!(state, len);
	Control::Continue(1)
}

/// Copy input data in current environment to memory
pub fn calldatacopy(state: &mut Machine) -> Control {
	pop_u256!(state, memory_offset, data_offset, len);

	let memory_offset = as_usize_or_fail!(memory_offset);
	// Clamp before narrowing, not after: see codecopy above.
	let data_offset = min(data_offset, U256::from(state.data.len())).as_usize();
	let len = as_usize_or_fail!(len);

	if len == 0 {
		return Control::Continue(1)
	}

	try_or_fail!(state.memory.resize_offset(memory_offset, len));
	match state.memory.copy_large(memory_offset, data_offset, len, &state.data) {
		Ok(()) => Control::Continue(1),
		Err(e) => Control::Exit(e.into()),
	}
}

/// Remove item from stack
pub fn pop(state: &mut Machine) -> Control {
	pop_u256!(state, _val);
	Control::Continue(1)
}

/// Load word from memory
pub fn mload(state: &mut Machine) -> Control {
	pop_u256!(state, index);
	let index = as_usize_or_fail!(index);
	try_or_fail!(state.memory.resize_offset(index, 32));
	let value = state.memory.load_h256(index);
	push!(state, value);
	Control::Continue(1)
}

/// Save word to memory
pub fn mstore(state: &mut Machine) -> Control {
	pop_u256!(state, index);
	pop!(state, value);
	let index = as_usize_or_fail!(index);
	try_or_fail!(state.memory.resize_offset(index, 32));
	match state.memory.set(index, &value[..], Some(32)) {
		Ok(()) => Control::Continue(1),
		Err(e) => Control::Exit(e.into()),
	}
}

/// Copy memory areas
pub fn mcopy(state: &mut Machine) -> Control {
	pop_u256!(state, dst_offset, src_offset, size);

	let dst_offset = as_usize_or_fail!(dst_offset);
	let src_offset = as_usize_or_fail!(src_offset);
	let size = as_usize_or_fail!(size);

	try_or_fail!(state.memory.resize_offset(max(src_offset, dst_offset), size));

	let value = state.memory.get(src_offset, size);
	match state.memory.set(dst_offset, &value, Some(size)) {
		Ok(()) => Control::Continue(1),
		Err(e) => Control::Exit(e.into()),
	}
}

/// Save byte to memory
pub fn mstore8(state: &mut Machine) -> Control {
	pop_u256!(state, index, value);
	let index = as_usize_or_fail!(index);
	try_or_fail!(state.memory.resize_offset(index, 1));
	#[allow(clippy::cast_possible_truncation)]
	let value = (value.low_u32() & 0xff) as u8;
	match state.memory.set(index, &[value], Some(1)) {
		Ok(()) => Control::Continue(1),
		Err(e) => Control::Exit(e.into()),
	}
}

/// Alter the program counter
pub fn jump(state: &mut Machine) -> Control {
	pop_u256!(state, dest);
	let dest = as_usize_or_fail!(dest, ExitError::InvalidJump);

	if state.valids.is_valid(dest) {
		Control::Jump(dest)
	} else {
		Control::Exit(ExitError::InvalidJump.into())
	}
}

/// Conditionally alter the program counter
pub fn jumpi(state: &mut Machine) -> Control {
	pop_u256!(state, dest, value);
	let dest = as_usize_or_fail!(dest, ExitError::InvalidJump);

	if value == U256::zero() {
		Control::Continue(1)
	} else {
		if state.valids.is_valid(dest) {
			Control::Jump(dest)
		} else {
			Control::Exit(ExitError::InvalidJump.into())
		}
	}
}

/// Get the value of the program counter prior to the increment corresponding to this instruction
pub fn pc(state: &mut Machine, position: usize) -> Control {
	push_u256!(state, U256::from(position));
	Control::Continue(1)
}

/// Get the size of active memory in bytes
pub fn msize(state: &mut Machine) -> Control {
	push_u256!(state, U256::from(state.memory.effective_len()));
	Control::Continue(1)
}

/// Place byte item on a stack
pub fn push(state: &mut Machine, n: usize, position: usize) -> Control {
	let end = min(position + 1 + n, state.code.len());
	let val = U256::from_big_endian_fast(&state.code[(position + 1)..end]);

	push_u256!(state, val);
	Control::Continue(1 + n)
}

/// Place value 0 on stack
pub fn push0(state: &mut Machine) -> Control {
	push_u256!(state, U256::zero());
	Control::Continue(1)
}

/// Duplicate stack item
pub fn dup(state: &mut Machine, n: usize) -> Control {
	if let Err(e) = state.stack.dup(n - 1) {
		return Control::Exit(e.into());
	};

	Control::Continue(1)
}

// Exchange stack items
pub fn swap(state: &mut Machine, n: usize) -> Control {
	if let Err(e) = state.stack.swap(n) {
		return Control::Exit(e.into());
	};

	Control::Continue(1)
}

/// Halt execution returning output data
pub fn ret(state: &mut Machine) -> Control {
	pop_u256!(state, start, len);
	let start = as_usize_or_fail!(start);
	let len = as_usize_or_fail!(len);
	try_or_fail!(state.memory.resize_offset(start, len));
	state.return_range = (start, len);
	Control::Exit(ExitSucceed::Returned.into())
}

/// Halt execution reverting state changes but returning data and remaining gas
pub fn revert(state: &mut Machine) -> Control {
	pop_u256!(state, start, len);
	let start = as_usize_or_fail!(start);
	let len = as_usize_or_fail!(len);
	try_or_fail!(state.memory.resize_offset(start, len));
	state.return_range = (start, len);
	Control::Exit(ExitRevert::Reverted.into())
}
