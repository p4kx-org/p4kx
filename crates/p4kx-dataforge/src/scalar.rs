// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{DataForge, DataType, Guid, Value, ValueError};

/// An original admission input of a decoded textual scalar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScalarSource {
    /// The original `EnumCount` header word.
    EnumCount,
    /// The original text length header word.
    TextLength,
    /// The original terminal NUL of a nonempty string.
    #[non_exhaustive]
    TextTerminator {
        /// Inclusive start in the decoded DCB.
        start: u64,
        /// Exclusive end in the decoded DCB.
        end: u64,
    },
    /// Consulted original text bytes that did not decode.
    #[non_exhaustive]
    AttemptedText {
        /// Inclusive start in the decoded DCB.
        start: u64,
        /// Exclusive end in the decoded DCB.
        end: u64,
    },
    /// The stored offset does not reach readable text bytes.
    #[non_exhaustive]
    UnavailableText {
        /// Original offset relative to the text table.
        offset: u32,
    },
}

type ObserveScalar<'observer> = &'observer mut dyn FnMut(ScalarSource);

/// Reads a scalar of `data_type` from its stored bytes, or returns `None` for a data type that
/// is no scalar; `target` is the enum an enum value belongs to.
pub(crate) fn read_scalar<'f>(
    forge: &'f DataForge<'f>,
    data_type: DataType,
    target: u16,
    bytes: &[u8],
) -> Option<Value<'f>> {
    read_scalar_with(forge, data_type, target, bytes, None)
}

pub(crate) fn read_scalar_with<'f>(
    forge: &'f DataForge<'f>,
    data_type: DataType,
    target: u16,
    bytes: &[u8],
    mut observe: Option<ObserveScalar<'_>>,
) -> Option<Value<'f>> {
    Some(match data_type {
        DataType::Bool => match first_bytes::<1>(bytes) {
            [0] => Value::Bool(false),
            [1] => Value::Bool(true),
            [byte] => Value::Error(ValueError::InvalidBool { byte }),
        },
        DataType::Int8 => Value::Int8(i8::from_le_bytes(first_bytes(bytes))),
        DataType::Int16 => Value::Int16(i16::from_le_bytes(first_bytes(bytes))),
        DataType::Int32 => Value::Int32(i32::from_le_bytes(first_bytes(bytes))),
        DataType::Int64 => Value::Int64(i64::from_le_bytes(first_bytes(bytes))),
        DataType::UInt8 => Value::UInt8(u8::from_le_bytes(first_bytes(bytes))),
        DataType::UInt16 => Value::UInt16(u16::from_le_bytes(first_bytes(bytes))),
        DataType::UInt32 => Value::UInt32(u32::from_le_bytes(first_bytes(bytes))),
        DataType::UInt64 => Value::UInt64(u64::from_le_bytes(first_bytes(bytes))),
        DataType::Single => Value::Single(f32::from_le_bytes(first_bytes(bytes))),
        DataType::Double => Value::Double(f64::from_le_bytes(first_bytes(bytes))),
        DataType::Guid => Value::Guid(Guid::from_stored(first_bytes(bytes))),
        DataType::String => text(forge, bytes, Value::String, observe),
        DataType::Locale => text(forge, bytes, Value::Locale, observe),
        DataType::Enum => {
            if let Some(observe) = observe.as_deref_mut() {
                observe(ScalarSource::EnumCount);
            }
            if usize::from(target) < forge.enums.len() {
                text(forge, bytes, Value::Enum, observe)
            } else {
                Value::Error(ValueError::UnknownEnum { index: target })
            }
        }
        DataType::Class | DataType::StrongPointer | DataType::WeakPointer | DataType::Reference => {
            return None;
        }
    })
}

fn text<'f>(
    forge: &'f DataForge<'f>,
    bytes: &[u8],
    wrap: fn(&'f str) -> Value<'f>,
    observe: Option<ObserveScalar<'_>>,
) -> Value<'f> {
    let offset = u32::from_le_bytes(first_bytes(bytes));
    let Some(observe) = observe else {
        return forge.text.value(offset).map_or_else(Value::Error, wrap);
    };
    observe(ScalarSource::TextLength);
    let (value, examined) = forge.text.value_with_extent(offset);
    match (&value, examined) {
        (Ok(text), Some(bytes)) if !text.is_empty() => {
            if let Some(span) = bytes
                .last_chunk::<1>()
                .and_then(|byte| forge.byte_span(byte))
            {
                observe(ScalarSource::TextTerminator {
                    start: span.start,
                    end: span.end,
                });
            }
        }
        (Err(_), examined) => observe(examined.and_then(|bytes| forge.byte_span(bytes)).map_or(
            ScalarSource::UnavailableText { offset },
            |span| ScalarSource::AttemptedText {
                start: span.start,
                end: span.end,
            },
        )),
        _ => {}
    }
    value.map_or_else(Value::Error, wrap)
}

/// Returns the first `N` bytes, zero-filled when fewer are given.
pub(crate) fn first_bytes<const N: usize>(bytes: &[u8]) -> [u8; N] {
    bytes.first_chunk::<N>().copied().unwrap_or([0; N])
}
