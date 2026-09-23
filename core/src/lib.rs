//! Core layer for EVM.

#![deny(warnings)]
#![forbid(unused_variables)]
// TODO: to pay attention
// #![forbid(unused_variables, unused_imports)]
#![deny(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(
	clippy::module_name_repetitions,
	clippy::missing_errors_doc,
	clippy::missing_panics_doc
)]

#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;
extern crate alloc;

mod memory;
mod stack;
mod valids;
mod opcode;
mod error;
mod eval;
mod utils;
mod primitive_types;
mod context;
mod rc_bytes;

pub use crate::memory::Memory;
pub use crate::stack::Stack;
pub use crate::valids::Valids;
pub use crate::opcode::Opcode;
pub use crate::error::{Trap, Capture, ExitReason, ExitSucceed, ExitError, ExitRevert, ExitFatal};
pub use crate::primitive_types::{H160, H256, U256, U512};
pub use crate::context::{Context, CreateScheme, CallScheme, Transfer};

use alloc::{rc::Rc, vec::Vec};
use crate::eval::{eval, Control};

/// Core execution layer for EVM.
#[cfg_attr(feature = "with-codec", derive(codec::Encode, codec::Decode))]
#[cfg_attr(feature = "with-serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct Machine {
	/// Program data.
	#[cfg_attr(feature = "with-serde", serde(with = "serde_bytes"))]
	data: Vec<u8>,
	/// Program code. Shared: every frame of the same address in one execution holds an `Rc`
	/// to the one copy the caller made (`new_shared`); on the wire it is the byte vector.
	#[cfg_attr(feature = "with-serde", serde(with = "rc_bytes"))]
	#[borsh(serialize_with = "rc_bytes::borsh_serialize", deserialize_with = "rc_bytes::borsh_deserialize")]
	code: Rc<Vec<u8>>,
	/// Program counter.
	position: Result<usize, ExitReason>,
	/// Return value.
	return_range: (usize, usize),
	/// Code validity maps.
	valids: Valids,
	/// Memory.
	memory: Memory,
	/// Stack.
	stack: Stack,
}

impl Machine {
	/// Reference of machine stack.
	#[must_use]
	pub const fn stack(&self) -> &Stack { &self.stack }
	/// Mutable reference of machine stack.
	pub fn stack_mut(&mut self) -> &mut Stack { &mut self.stack }
	/// Reference of machine memory.
	#[must_use]
	pub const fn memory(&self) -> &Memory { &self.memory }
	/// Mutable reference of machine memory.
	pub fn memory_mut(&mut self) -> &mut Memory { &mut self.memory }

        /// Return a reference of the program counter.
        pub fn position(&self) -> &Result<usize, ExitReason> {
                &self.position
        }

	/// Create a new machine with given code and data.
	#[must_use]
	pub fn new(
		code: Vec<u8>,
		valids: Vec<u8>,
		data: Vec<u8>,
		stack_limit: usize,
		memory_limit: usize
	) -> Self {
		Self::new_shared(Rc::new(code), Rc::new(valids), data, stack_limit, memory_limit)
	}

	/// Create a new machine over code and valids shared with the other frames of the same
	/// address: the caller copies a contract's code out of its account once per execution
	/// and every frame that runs it holds an `Rc` to that one copy.
	#[must_use]
	pub fn new_shared(
		code: Rc<Vec<u8>>,
		valids: Rc<Vec<u8>>,
		data: Vec<u8>,
		stack_limit: usize,
		memory_limit: usize
	) -> Self {
		Self {
			data,
			code,
			position: Ok(0),
			return_range: (0, 0),
			valids: Valids::shared(valids),
			memory: Memory::new(memory_limit),
			stack: Stack::new(stack_limit),
		}
	}

	/// Explicit exit of the machine. Further step will return error.
	pub fn exit(&mut self, reason: ExitReason) {
		self.position = Err(reason);
	}

	/// Inspect the machine's next opcode and current stack.
	#[must_use]
	pub fn inspect(&self) -> Option<(Opcode, &Stack)> {
		let position = match self.position {
			Ok(position) => position,
			Err(_) => return None,
		};
		self.code.get(position).map(|v| (Opcode(*v), &self.stack))
	}

	/// Gets return value len by `return_range`
	#[must_use]
	pub fn return_value_len(&self) -> usize {
		self.return_range.1
	}

	/// Copy and get the return value of the machine, if any.
	#[must_use]
	pub fn return_value(&self) -> Vec<u8> {
		self.memory.get(
			self.return_range.0,
			self.return_range.1,
		)
	}

	/// Loop stepping the machine, until it stops.
	pub fn run<F>(&mut self,
				  max_steps: u64,
				  mut pre_validate: F,
				  _context : &Context
	) -> (u64, Capture<ExitReason, Trap>)
		where F: FnMut(Opcode, &Stack) -> Result<(), ExitError>
	{
		for step in 0..max_steps {
			let position = match self.position {
				Ok(position) => position,
				Err(reason) => return (step, Capture::Exit(reason))
			};

			let opcode = match self.code.get(position) {
				Some(opcode) => Opcode(*opcode),
				None => {
					self.position = Err(ExitReason::Succeed(ExitSucceed::Stopped));
					return (step, Capture::Exit(ExitReason::Succeed(ExitSucceed::Stopped)));
				}
			};

			if let Err(error) = pre_validate(opcode, &self.stack()) {
				let reason = ExitReason::from(error);
				self.exit(reason);
				return (step, Capture::Exit(reason));
			}

			let result = match eval(self, opcode, position) {
				Control::Continue(p) => {
					self.position = Ok(position + p);
					Ok(())
				},
				Control::Exit(reason) => {
					self.exit(reason);
					Err(Capture::Exit(reason))
				},
				Control::Jump(p) => {
					self.position = Ok(p);
					Ok(())
				},
				Control::Trap(opcode) => {
					self.position = Ok(position + 1);
					Err(Capture::Trap(opcode))
				},
			};

			if let Err(capture) = result {
				return (step, capture)
			}
		}

		(max_steps, Capture::Exit(ExitReason::StepLimitReached))
	}

}
