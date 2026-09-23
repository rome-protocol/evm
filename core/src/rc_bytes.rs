//! An `Rc<Vec<u8>>` on the wire is the byte vector it holds, exactly as when `Machine` and
//! `Valids` owned their bytes: a `u32` length and the bytes under Borsh, `serde_bytes` under
//! serde. The code and valids of one address are shared between the frames that run it
//! (`Machine::new_shared`); a deserialized frame gets its own copy, as it always did.

use alloc::{rc::Rc, vec::Vec};
use borsh::{BorshDeserialize, BorshSerialize};

pub fn borsh_serialize<W: borsh::io::Write>(bytes: &Rc<Vec<u8>>, writer: &mut W) -> Result<(), borsh::io::Error> {
	let bytes: &Vec<u8> = bytes;
	BorshSerialize::serialize(bytes, writer)
}

pub fn borsh_deserialize<R: borsh::io::Read>(reader: &mut R) -> Result<Rc<Vec<u8>>, borsh::io::Error> {
	Vec::<u8>::deserialize_reader(reader).map(Rc::new)
}

#[cfg(feature = "with-serde")]
pub fn serialize<S: serde::Serializer>(bytes: &Rc<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error> {
	let bytes: &Vec<u8> = bytes;
	serde_bytes::serialize(bytes, serializer)
}

#[cfg(feature = "with-serde")]
pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Rc<Vec<u8>>, D::Error> {
	serde_bytes::deserialize::<Vec<u8>, D>(deserializer).map(Rc::new)
}
