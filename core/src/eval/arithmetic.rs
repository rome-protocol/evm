use core::ops::Rem;
use core::convert::TryInto;
use crate::{utils::I256, U256, U512};

/// Integer division operation
pub fn div(op1: U256, op2: U256) -> U256 {
	if op2 == U256::zero() {
		U256::zero()
	} else {
		op1 / op2
	}
}

/// Signed integer division operation (truncated)
pub fn sdiv(op1: U256, op2: U256) -> U256 {
	let op1: I256 = op1.into();
	let op2: I256 = op2.into();
	let ret = op1 / op2;
	ret.into()
}

/// Modulo remainder operation
pub fn rem(op1: U256, op2: U256) -> U256 {
	if op2 == U256::zero() {
		U256::zero()
	} else {
		op1.rem(op2)
	}
}

/// Signed modulo remainder operation
pub fn srem(op1: U256, op2: U256) -> U256 {
	if op2 == U256::zero() {
		U256::zero()
	} else {
		let op1: I256 = op1.into();
		let op2: I256 = op2.into();
		let ret = op1.rem(op2);
		ret.into()
	}
}

/// Modulo addition operation
pub fn addmod(op1: U256, op2: U256, op3: U256) -> U256 {
	let op1: U512 = op1.into();
	let op2: U512 = op2.into();
	let op3: U512 = op3.into();

	if op3 == U512::zero() {
		U256::zero()
	} else {
		let v = (op1 + op2) % op3;
		v.try_into().expect("op3 is less than U256::max_value(), thus it never overflows; qed")
	}
}

/// Modulo multiplication operation
pub fn mulmod(op1: U256, op2: U256, op3: U256) -> U256 {
	let op1: U512 = op1.into();
	let op2: U512 = op2.into();
	let op3: U512 = op3.into();

	if op3 == U512::zero() {
		U256::zero()
	} else {
		let v = (op1 * op2) % op3;
		v.try_into().expect("op3 is less than U256::max_value(), thus it never overflows; qed")
	}
}

/// Exponential operation
pub fn exp(op1: U256, op2: U256) -> U256 {
	let mut op1 = op1;
	let mut op2 = op2;
	let mut r: U256 = 1.into();

	while op2 != 0.into() {
		if op2 & 1.into() != 0.into() {
			r = r.overflowing_mul(op1).0;
		}
		op2 >>= 1;
		op1 = op1.overflowing_mul(op1).0;
	}

	r
}

/// Extend length of two’s complement signed integer
pub fn signextend(op1: U256, op2: U256) -> U256 {
	// Byte 31 spans the whole word, so the result is op2; building its mask
	// would underflow `(1 << 256) - 1`.
	if op1 >= U256::from(31) {
		op2
	} else {
		let byte_index = op1.as_usize();
		let bit_index = 8 * byte_index + 7;

		if op2.bit(bit_index) {
			// Sign bit is 1 → extend with 1s
			let mask = U256::MAX << (bit_index + 1);
			op2 | mask
		} else {
			// Sign bit is 0 → zero out upper bits
			let mask = (U256::one() << (bit_index + 1)) - 1;
			op2 & mask
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn signextend_at_byte_31_with_sign_bit_clear_is_identity_not_panic() {
		assert_eq!(signextend(U256::from(31), U256::from(5)), U256::from(5));
	}

	#[test]
	fn signextend_at_byte_31_with_sign_bit_set_is_identity() {
		let value = (U256::one() << 255) | U256::from(7);
		assert_eq!(signextend(U256::from(31), value), value);
	}

	#[test]
	fn signextend_in_range_unaffected() {
		assert_eq!(signextend(U256::from(30), U256::from(5)), U256::from(5));
		assert_eq!(signextend(U256::zero(), U256::from(0xff)), U256::MAX);
		assert_eq!(signextend(U256::zero(), U256::from(0x7f)), U256::from(0x7f));
		assert_eq!(signextend(U256::from(32), U256::from(5)), U256::from(5));
	}
}
