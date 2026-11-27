// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

use super::compound::Compound;
use super::{Emitter, Error, Token};

macro_rules! number {
    ($name:ident, $ty:ty, $variant:ident) => {
        fn $name(self, value: $ty) -> Result<(), Error> {
            self.emit(Token::$variant(value.into()));
            Ok(())
        }
    };
}

impl<'a, 't> serde::Serializer for &'a mut Emitter<'t> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Compound<'a, 't>;
    type SerializeTuple = Compound<'a, 't>;
    type SerializeTupleStruct = Compound<'a, 't>;
    type SerializeTupleVariant = Compound<'a, 't>;
    type SerializeMap = Compound<'a, 't>;
    type SerializeStruct = Compound<'a, 't>;
    type SerializeStructVariant = Compound<'a, 't>;

    number!(serialize_i8, i8, Signed);
    number!(serialize_i16, i16, Signed);
    number!(serialize_i32, i32, Signed);
    number!(serialize_i64, i64, Signed);
    fn serialize_i128(self, value: i128) -> Result<(), Error> {
        self.emit(Token::Signed(value));
        Ok(())
    }
    number!(serialize_u8, u8, Unsigned);
    number!(serialize_u16, u16, Unsigned);
    number!(serialize_u32, u32, Unsigned);
    number!(serialize_u64, u64, Unsigned);
    fn serialize_u128(self, value: u128) -> Result<(), Error> {
        self.emit(Token::Unsigned(value));
        Ok(())
    }

    fn serialize_bool(self, value: bool) -> Result<(), Error> {
        self.emit(Token::Bool(value));
        Ok(())
    }
    fn serialize_f32(self, value: f32) -> Result<(), Error> {
        self.emit(Token::Float32(value.to_bits()));
        Ok(())
    }
    fn serialize_f64(self, value: f64) -> Result<(), Error> {
        self.emit(Token::Float64(value.to_bits()));
        Ok(())
    }
    fn serialize_char(self, value: char) -> Result<(), Error> {
        self.emit(Token::Character(value));
        Ok(())
    }
    fn serialize_str(self, value: &str) -> Result<(), Error> {
        self.emit(Token::Text(value.to_owned()));
        Ok(())
    }
    fn serialize_bytes(self, value: &[u8]) -> Result<(), Error> {
        self.emit(Token::Bytes(value.to_owned()));
        Ok(())
    }
    fn serialize_none(self) -> Result<(), Error> {
        self.emit(Token::None);
        Ok(())
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<(), Error> {
        self.emit(Token::Some);
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Error> {
        self.emit(Token::Unit);
        Ok(())
    }
    fn serialize_unit_struct(self, name: &'static str) -> Result<(), Error> {
        self.emit(Token::Label(name));
        self.serialize_unit()
    }
    fn serialize_unit_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
    ) -> Result<(), Error> {
        self.emit(Token::Variant(name, index, variant));
        self.serialize_unit()
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.emit(Token::Label(name));
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.emit(Token::Variant(name, index, variant));
        value.serialize(self)
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        Ok(Compound::new(self, "sequence", len))
    }
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Error> {
        Ok(Compound::new(self, "tuple", Some(len)))
    }
    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        self.emit(Token::Label(name));
        Ok(Compound::new(self, "tupleStruct", Some(len)))
    }
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        self.emit(Token::Variant(name, index, variant));
        Ok(Compound::new(self, "tupleVariant", Some(len)))
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Ok(Compound::new(self, "map", len))
    }
    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Error> {
        self.emit(Token::Label(name));
        Ok(Compound::new(self, "struct", Some(len)))
    }
    fn serialize_struct_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        self.emit(Token::Variant(name, index, variant));
        Ok(Compound::new(self, "structVariant", Some(len)))
    }
}
