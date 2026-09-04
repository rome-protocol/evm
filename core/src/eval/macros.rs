macro_rules! try_or_fail {
	( $e:expr ) => {
		match $e {
			Ok(v) => v,
			Err(e) => return Control::Exit(e.into())
		}
	}
}

macro_rules! pop {
	( $machine:expr, $( $x:ident ),* ) => (
		$(
			let $x = match $machine.stack.pop() {
				Ok(value) => value,
				Err(e) => return Control::Exit(e.into()),
			};
		)*
	);
}

macro_rules! pop_u256 {
	( $machine:expr, $( $x:ident ),* ) => (
		$(
			let $x = match $machine.stack.pop_u256() {
				Ok(value) => value,
				Err(e) => return Control::Exit(e.into()),
			};
		)*
	);
}

/// push H256 to stack
macro_rules! push {
	( $machine:expr, $( $x:expr ),* ) => (
		$(
			match $machine.stack.push($x) {
				Ok(()) => (),
				Err(e) => return Control::Exit(e.into()),
			}
		)*
	)
}

/// push U256 to stack
macro_rules! push_u256 {
	( $machine:expr, $( $x:expr ),* ) => (
		$(
			match $machine.stack.push_u256($x) {
				Ok(()) => (),
				Err(e) => return Control::Exit(e.into()),
			}
		)*
	)
}

/// call fn(v)
macro_rules! op1_u256_fn {
	( $machine:expr, $op:path ) => (
		{
			pop_u256!($machine, op1);
			let ret = $op(op1);
			push_u256!($machine, ret);

			Control::Continue(1)
		}
	)
}

/// compare v1, v2
macro_rules! op2_u256_bool_ref {
	( $machine:expr, $op:ident ) => (
		{
			pop_u256!($machine, op1, op2);
			let ret = op1.$op(&op2);
			push_u256!($machine, if ret {
				U256::one()
			} else {
				U256::zero()
			});

			Control::Continue(1)
		}
	)
}

/// v1.fn(v2)
macro_rules! op2_u256 {
	( $machine:expr, $op:ident ) => (
		{
			pop_u256!($machine, op1, op2);
			let ret = op1.$op(op2);
			push_u256!($machine, ret);

			Control::Continue(1)
		}
	)
}

/// (a, b) = v1.fn(v2),
macro_rules! op2_u256_tuple {
	( $machine:expr, $op:ident ) => (
		{
			pop_u256!($machine, op1, op2);
			let (ret, ..) = op1.$op(op2);
			push_u256!($machine, ret);

			Control::Continue(1)
		}
	)
}

/// fn(v1, v2)
macro_rules! op2_u256_fn {
	( $machine:expr, $op:path ) => (
		{
			pop_u256!($machine, op1, op2);
			let ret = $op(op1, op2);
			push_u256!($machine, ret);

			Control::Continue(1)
		}
	)
}

/// fn(v1, v2, v3)
macro_rules! op3_u256_fn {
	( $machine:expr, $op:path ) => (
		{
			pop_u256!($machine, op1, op2, op3);
			let ret = $op(op1, op2, op3);
			push_u256!($machine, ret);

			Control::Continue(1)
		}
	)
}

/// cast U256 to usize
macro_rules! as_usize_or_fail {
	( $v:expr ) => {
		{
			// Frame-local, not ExitFatal: an operand too large to represent as
			// a memory offset is the same class of failure a gasometer would
			// have produced for an unaffordable expansion (OutOfGas), and the
			// caller can recover from it like any other ExitError.
			if $v > U256::from(usize::max_value()) {
				return Control::Exit(ExitError::OutOfGas.into())
			}

			$v.as_usize()
		}
	};

	( $v:expr, $reason:expr ) => {
		{
			if $v > U256::from(usize::max_value()) {
				return Control::Exit($reason.into())
			}

			$v.as_usize()
		}
	};
}
