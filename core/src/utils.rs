#![allow(clippy::use_self)]

use core::ops::{Rem, Div};
use core::cmp::Ordering;
use crate::U256;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[allow(clippy::pub_enum_variant_names)]
pub enum Sign {
	Plus,
	Minus,
	NoSign,
}

const SIGN_BIT_MASK: U256 = U256([0xffff_ffff_ffff_ffff, 0xffff_ffff_ffff_ffff,
								  0xffff_ffff_ffff_ffff, 0x7fff_ffff_ffff_ffff]);

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct I256(pub Sign, pub U256);

impl I256 {
	/// Zero value of I256.
	pub const fn zero() -> I256 { I256(Sign::NoSign, U256::zero()) }
	/// Minimum value of I256.
	pub fn min_value() -> I256 { I256(Sign::Minus, (U256::max_value() & SIGN_BIT_MASK) + U256::from(1_u64)) }
}

impl Ord for I256 {
	fn cmp(&self, other: &I256) -> Ordering {
		#[allow(clippy::match_same_arms)]
		match (self.0, other.0) {
			(Sign::NoSign, Sign::NoSign) => Ordering::Equal,
			(Sign::NoSign, Sign::Plus) => Ordering::Less,
			(Sign::NoSign, Sign::Minus) => Ordering::Greater,
			(Sign::Minus, Sign::NoSign) => Ordering::Less,
			(Sign::Minus, Sign::Plus) => Ordering::Less,
			(Sign::Minus, Sign::Minus) => self.1.cmp(&other.1).reverse(),
			(Sign::Plus, Sign::Minus) => Ordering::Greater,
			(Sign::Plus, Sign::NoSign) => Ordering::Greater,
			(Sign::Plus, Sign::Plus) => self.1.cmp(&other.1),
		}
	}
}

impl PartialOrd for I256 {
	fn partial_cmp(&self, other: &I256) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Default for I256 { fn default() -> I256 { I256::zero() } }
impl From<U256> for I256 {
	fn from(val: U256) -> I256 {
		if val == U256::zero() {
			I256::zero()
		} else if val & SIGN_BIT_MASK == val {
			I256(Sign::Plus, val)
		} else {
			I256(Sign::Minus, !val + U256::from(1_u64))
		}
	}
}
#[allow(clippy::from_over_into)]
impl Into<U256> for I256 {
	fn into(self) -> U256 {
		let sign = self.0;
		if sign == Sign::NoSign {
			U256::zero()
		} else if sign == Sign::Plus {
			self.1
		} else {
			!self.1 + U256::from(1_u64)
		}
	}
}

impl Div for I256 {
	type Output = I256;

	fn div(self, other: I256) -> I256 {
		if other == I256::zero() {
			return I256::zero();
		}

		// The spec's explicit overflow case. Redundant with the general path
		// below (MIN/-1 yields magnitude 2^255, which round-trips to MIN), kept
		// because the spec states it outright.
		if self == I256::min_value() && other == I256(Sign::Minus, U256::from(1_u64)) {
			return I256::min_value();
		}

		// Magnitudes are bounded: self.1/other.1 <= 2^255, with equality only at
		// MIN/+-1 (both already handled above/below this line). Masking bit 255
		// here collapsed that single legal quotient to zero (SDIV(MIN,1) -> 0
		// instead of MIN) — the mask served no other case, so it is removed
		// rather than special-cased further.
		let d = self.1 / other.1;

		if d == U256::zero() {
			return I256::zero();
		}

		match (self.0, other.0) {
			(Sign::Plus, Sign::Plus) |
			(Sign::Minus, Sign::Minus) => I256(Sign::Plus, d),
			(Sign::Plus, Sign::Minus) |
			(Sign::Minus, Sign::Plus) => I256(Sign::Minus, d),
			_ => I256::zero()
		}
	}
}

impl Rem for I256 {
	type Output = I256;

	fn rem(self, other: I256) -> I256 {
		let r = (self.1 % other.1) & SIGN_BIT_MASK;

		if r == U256::zero() {
			return I256::zero()
		}

		I256(self.0, r)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn min_u256() -> U256 { U256::one() << 255 }

	// Mirrors `eval::arithmetic::sdiv`/`srem` (the opcode-level wrappers), which
	// guard division-by-zero before ever reaching `I256`. `Div` already carries
	// its own zero check (`other == I256::zero()` at the top); `Rem` does not
	// (a pre-existing, out-of-scope gap — the finding is the Div mask only), so
	// the b==0 guard here is required to avoid hitting it.
	fn sdiv(a: U256, b: U256) -> U256 {
		if b.is_zero() { U256::zero() } else { (I256::from(a) / I256::from(b)).into() }
	}
	fn srem(a: U256, b: U256) -> U256 {
		if b.is_zero() { U256::zero() } else { (I256::from(a) % I256::from(b)).into() }
	}

	#[test]
	fn sdiv_min_by_one_is_min() {
		// Regression: `& SIGN_BIT_MASK` on the quotient collapses the one legal
		// magnitude of exactly 2^255 (MIN/+-1) to zero.
		assert_eq!(sdiv(min_u256(), U256::one()), min_u256());
	}

	#[test]
	fn sdiv_min_by_minus_one_is_min() {
		assert_eq!(sdiv(min_u256(), U256::max_value()), min_u256());
	}

	#[test]
	fn sdiv_min_by_two_is_minus_two_pow_254() {
		let expected: U256 = I256(Sign::Minus, U256::one() << 254).into();
		assert_eq!(sdiv(min_u256(), U256::from(2)), expected);
	}

	#[test]
	fn smod_min_by_minus_one_is_zero() {
		assert_eq!(srem(min_u256(), U256::max_value()), U256::zero());
	}

	fn from_i128(v: i128) -> U256 {
		if v >= 0 {
			U256::from(v as u128)
		} else {
			let mag = U256::from(v.unsigned_abs());
			(!mag).overflowing_add(U256::one()).0
		}
	}

	// i128::MIN / -1 overflows i128 (magnitude 2^127 doesn't fit back into i128)
	// but is a perfectly ordinary division in 256-bit space (int256 range dwarfs
	// i128), so the wide-space reference is computed directly in U256 rather
	// than routed back through i128.
	fn ref_sdiv_wide(a: i128, b: i128) -> U256 {
		if b == 0 {
			return U256::zero();
		}
		if a == i128::MIN && b == -1 {
			return U256::from(i128::MIN.unsigned_abs());
		}
		from_i128(a / b)
	}

	fn ref_srem_wide(a: i128, b: i128) -> U256 {
		if b == 0 || (a == i128::MIN && b == -1) {
			return U256::zero();
		}
		from_i128(a % b)
	}

	#[test]
	fn sdiv_srem_sweep_matches_i128_reference() {
		let values: [i128; 12] = [
			i128::MIN, i128::MIN + 1, -1_000_000, -7, -2, -1, 0, 1, 2, 7, 1_000_000, i128::MAX,
		];

		for &a in &values {
			for &b in &values {
				let got_div = sdiv(from_i128(a), from_i128(b));
				let want_div = ref_sdiv_wide(a, b);
				assert_eq!(got_div, want_div, "SDIV({a}, {b})");

				let got_rem = srem(from_i128(a), from_i128(b));
				let want_rem = ref_srem_wide(a, b);
				assert_eq!(got_rem, want_rem, "SMOD({a}, {b})");
			}
		}
	}
}