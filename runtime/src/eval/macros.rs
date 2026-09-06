/// check the value
macro_rules! try_or_fail {
	( $e:expr ) => {
		match $e {
			Ok(v) => v,
			Err(e) => return Control::Exit(e.into())
		}
	}
}

// try to pop a H256 from stack
macro_rules! pop {
	( $machine:expr, $( $x:ident ),* ) => (
		$(
			let $x = match $machine.machine.stack_mut().pop() {
				Ok(value) => value,
				Err(e) => return Control::Exit(e.into()),
			};
		)*
	);
}


// try to pop a U256 from stack
macro_rules! pop_u256 {
	( $machine:expr, $( $x:ident ),* ) => (
		$(
			let $x = match $machine.machine.stack_mut().pop_u256() {
				Ok(value) => value,
				Err(e) => return Control::Exit(e.into()),
			};
		)*
	);
}

/// try to push H256 to the stack
macro_rules! push {
	( $machine:expr, $( $x:expr ),* ) => (
		$(
			match $machine.machine.stack_mut().push($x) {
				Ok(()) => (),
				Err(e) => return Control::Exit(e.into()),
			}
		)*
	)
}

/// try to push U256 to the stack
macro_rules! push_u256 {
	( $machine:expr, $( $x:expr ),* ) => (
		$(
			match $machine.machine.stack_mut().push_u256($x) {
				Ok(()) => (),
				Err(e) => return Control::Exit(e.into()),
			}
		)*
	)
}

/// cast to usize
macro_rules! as_usize_or_fail {
	( $v:expr ) => {
		{
			// Frame-local, not ExitFatal: see core/src/eval/macros.rs (same fix,
			// duplicated copy of this macro).
			if $v > U256::from(usize::max_value()) {
				return Control::Exit(ExitError::OutOfGas.into())
			}

			$v.as_usize()
		}
	};
}
