// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::ser::{
    Error as _, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
    SerializeTuple, SerializeTupleStruct, SerializeTupleVariant,
};
use serde::{Serialize, Serializer};

pub(crate) struct Finite<T>(pub(crate) T, pub(crate) u32);

struct Value<'a, T: ?Sized>(&'a T, u32);

impl<T> Finite<T> {
    fn child<E: serde::ser::Error>(&self) -> Result<u32, E> {
        self.1
            .checked_sub(1)
            .ok_or_else(|| E::custom("media value_depth budget exceeded"))
    }
}

impl<T: Serialize + ?Sized> Serialize for Value<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(Finite(serializer, self.1))
    }
}

macro_rules! primitives {
    ($($name:ident: $ty:ty),* $(,)?) => { $(
        fn $name(self, value: $ty) -> Result<Self::Ok, Self::Error> { self.0.$name(value) }
    )* };
}

impl<S: Serializer> Serializer for Finite<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = Finite<S::SerializeSeq>;
    type SerializeTuple = Finite<S::SerializeTuple>;
    type SerializeTupleStruct = Finite<S::SerializeTupleStruct>;
    type SerializeTupleVariant = Finite<S::SerializeTupleVariant>;
    type SerializeMap = Finite<S::SerializeMap>;
    type SerializeStruct = Finite<S::SerializeStruct>;
    type SerializeStructVariant = Finite<S::SerializeStructVariant>;

    primitives! { serialize_bool: bool, serialize_i8: i8, serialize_i16: i16, serialize_i32: i32,
    serialize_i64: i64, serialize_i128: i128, serialize_u8: u8, serialize_u16: u16,
    serialize_u32: u32, serialize_u64: u64, serialize_u128: u128, serialize_char: char,
    serialize_str: &str, serialize_bytes: &[u8] }

    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        if !value.is_finite() {
            return Err(S::Error::custom(
                "non-finite source float requires Float32 raw bits",
            ));
        }
        self.0.serialize_f32(value)
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        if !value.is_finite() {
            return Err(S::Error::custom(
                "non-finite source float requires Float64 raw bits",
            ));
        }
        self.0.serialize_f64(value)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_none()
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        let depth = self.child()?;
        self.0.serialize_some(&Value(value, depth))
    }
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit()
    }
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit_struct(name)
    }
    fn serialize_unit_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit_variant(name, index, variant)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        let depth = self.child()?;
        self.0.serialize_newtype_struct(name, &Value(value, depth))
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        let depth = self.child()?;
        self.0
            .serialize_newtype_variant(name, index, variant, &Value(value, depth))
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        let depth = self.child()?;
        self.0.serialize_seq(len).map(|inner| Finite(inner, depth))
    }
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        let depth = self.child()?;
        self.0
            .serialize_tuple(len)
            .map(|inner| Finite(inner, depth))
    }
    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        let depth = self.child()?;
        self.0
            .serialize_tuple_struct(name, len)
            .map(|inner| Finite(inner, depth))
    }
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        let depth = self.child()?;
        self.0
            .serialize_tuple_variant(name, index, variant, len)
            .map(|inner| Finite(inner, depth))
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        let depth = self.child()?;
        self.0.serialize_map(len).map(|inner| Finite(inner, depth))
    }
    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        let depth = self.child()?;
        self.0
            .serialize_struct(name, len)
            .map(|inner| Finite(inner, depth))
    }
    fn serialize_struct_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        let depth = self.child()?;
        self.0
            .serialize_struct_variant(name, index, variant, len)
            .map(|inner| Finite(inner, depth))
    }
    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }
}

macro_rules! sequence {
    ($trait:ident, $method:ident) => {
        impl<S: $trait> $trait for Finite<S> {
            type Ok = S::Ok;
            type Error = S::Error;
            fn $method<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
                self.0.$method(&Value(value, self.1))
            }
            fn end(self) -> Result<Self::Ok, Self::Error> {
                self.0.end()
            }
        }
    };
}

sequence!(SerializeSeq, serialize_element);
sequence!(SerializeTuple, serialize_element);
sequence!(SerializeTupleStruct, serialize_field);
sequence!(SerializeTupleVariant, serialize_field);

impl<S: SerializeMap> SerializeMap for Finite<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Self::Error> {
        self.0.serialize_key(&Value(key, self.1))
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.0.serialize_value(&Value(value, self.1))
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}

impl<S: SerializeStruct> SerializeStruct for Finite<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.0.serialize_field(key, &Value(value, self.1))
    }
    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        self.0.skip_field(key)
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}

impl<S: SerializeStructVariant> SerializeStructVariant for Finite<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.0.serialize_field(key, &Value(value, self.1))
    }
    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        self.0.skip_field(key)
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.0.end()
    }
}
