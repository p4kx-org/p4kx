// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

use super::{Emitter, Error, Token};

pub(super) struct Compound<'a, 't> {
    emitter: &'a mut Emitter<'t>,
}

impl<'a, 't> Compound<'a, 't> {
    pub(super) fn new(
        emitter: &'a mut Emitter<'t>,
        kind: &'static str,
        len: Option<usize>,
    ) -> Self {
        emitter.emit(Token::Start(kind));
        emitter.emit(Token::Length(len));
        Self { emitter }
    }

    fn value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        value.serialize(&mut *self.emitter)
    }

    fn field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<(), Error> {
        self.emitter.emit(Token::Label(key));
        self.value(value)
    }

    fn finish(self) {
        self.emitter.emit(Token::End);
    }
}

macro_rules! sequence {
    ($trait:ident, $method:ident) => {
        impl serde::ser::$trait for Compound<'_, '_> {
            type Ok = ();
            type Error = Error;
            fn $method<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
                self.value(value)
            }
            fn end(self) -> Result<(), Error> {
                self.finish();
                Ok(())
            }
        }
    };
}

macro_rules! structure {
    ($trait:ident) => {
        impl serde::ser::$trait for Compound<'_, '_> {
            type Ok = ();
            type Error = Error;
            fn serialize_field<T: Serialize + ?Sized>(
                &mut self,
                key: &'static str,
                value: &T,
            ) -> Result<(), Error> {
                self.field(key, value)
            }
            fn end(self) -> Result<(), Error> {
                self.finish();
                Ok(())
            }
        }
    };
}

sequence!(SerializeSeq, serialize_element);
sequence!(SerializeTuple, serialize_element);
sequence!(SerializeTupleStruct, serialize_field);
sequence!(SerializeTupleVariant, serialize_field);
structure!(SerializeStruct);
structure!(SerializeStructVariant);

impl serde::ser::SerializeMap for Compound<'_, '_> {
    type Ok = ();
    type Error = Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        self.value(key)
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.value(value)
    }
    fn end(self) -> Result<(), Error> {
        self.finish();
        Ok(())
    }
}
