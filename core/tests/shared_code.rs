//! Code shared across frames (rome-evm-canton, state shape step 6): a contract's code and its
//! jump-destination map are copied once per execution and every frame that runs the contract
//! holds an `Rc` to that copy (`Machine::new_shared`, `Runtime::new_shared`). The Borsh wire of
//! a shared machine is byte for byte the wire of one built from owned vectors.
//! Run: `cd core && cargo test --test shared_code`.

use borsh::{BorshDeserialize, BorshSerialize};
use evm_core::{Machine, Valids};
use std::rc::Rc;

fn code() -> Vec<u8> {
	// PUSH1 1 PUSH1 2 ADD JUMPDEST STOP, then every byte value once
	let mut c = vec![0x60, 0x01, 0x60, 0x02, 0x01, 0x5b, 0x00];
	c.extend(0u8..=255);
	c
}

#[test]
fn frames_of_one_address_hold_one_copy_of_its_code() {
	let code: Rc<[u8]> = Rc::from(code());
	let valids: Rc<[u8]> = Rc::from(Valids::compute(&code));
	let a = Machine::new_shared(code.clone(), valids.clone(), vec![], 1024, 10_000);
	let b = Machine::new_shared(code.clone(), valids.clone(), vec![], 1024, 10_000);
	assert_eq!(Rc::strong_count(&code), 3, "two frames and the caller's cache: one copy of the code");
	assert_eq!(Rc::strong_count(&valids), 3, "and one of the valids");
	drop(a);
	assert_eq!(Rc::strong_count(&code), 2);
	drop(b);
	assert_eq!((Rc::strong_count(&code), Rc::strong_count(&valids)), (1, 1));
}

#[test]
fn a_shared_machine_serializes_as_an_owning_one() {
	let code = code();
	let valids = Valids::compute(&code);
	let data = vec![0xaa; 36];
	let owned = Machine::new(code.clone(), valids.clone(), data.clone(), 1024, 10_000);
	let shared = Machine::new_shared(Rc::from(code), Rc::from(valids), data, 1024, 10_000);
	let mut w_owned = Vec::new();
	owned.serialize(&mut w_owned).unwrap();
	let mut w_shared = Vec::new();
	shared.serialize(&mut w_shared).unwrap();
	assert_eq!(w_owned, w_shared, "the wire is the byte vector, whoever holds it");
	let back = Machine::try_from_slice(&w_shared).unwrap();
	let mut again = Vec::new();
	back.serialize(&mut again).unwrap();
	assert_eq!(again, w_shared, "a deserialized frame round-trips");
}
