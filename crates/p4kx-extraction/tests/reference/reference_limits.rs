// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::marker::PhantomData;

use serde::de::{Deserialize, Deserializer, Error as _, IgnoredAny, SeqAccess, Visitor};

use super::reference_error::{Code, ReferenceError, Result};
use super::reference_log_report::Finding;
use super::reference_manifest::{
    Artifact, Expected, Location, MAX_ARTIFACTS, MAX_LOGICAL_SOURCES, MAX_PATH_BYTES,
};

const JSON_ESCAPE_EXPANSION: usize = 6;
const MAX_DETAILS: usize = 1000;

/// Bounds parser scratch for escaped metadata before Serde allocates decoded strings.
pub(crate) fn json_tokens(bytes: &[u8]) -> Result<()> {
    let mut quoted = false;
    let mut escaped = false;
    let mut token_bytes = 0_usize;
    for byte in bytes {
        if !quoted {
            if *byte == b'"' {
                quoted = true;
                token_bytes = 0;
            }
            continue;
        }
        if !escaped && *byte == b'"' {
            quoted = false;
            continue;
        }
        token_bytes += 1;
        if token_bytes > MAX_PATH_BYTES * JSON_ESCAPE_EXPANSION {
            return Err(ReferenceError::new(
                Code::Bounds,
                "JSON metadata token exceeds its scratch bound",
            ));
        }
        if escaped {
            escaped = false;
        } else {
            escaped = *byte == b'\\';
        }
    }
    Ok(())
}

struct Text;

impl Visitor<'_> for Text {
    type Value = String;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "metadata text of at most {MAX_PATH_BYTES} UTF-8 bytes"
        )
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> std::result::Result<String, E> {
        if value.len() > MAX_PATH_BYTES {
            return Err(E::custom("decoded metadata text exceeds its byte bound"));
        }
        Ok(value.to_owned())
    }

    fn visit_string<E: serde::de::Error>(self, value: String) -> std::result::Result<String, E> {
        if value.len() > MAX_PATH_BYTES {
            return Err(E::custom("decoded metadata text exceeds its byte bound"));
        }
        Ok(value)
    }
}

pub(crate) fn text<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<String, D::Error> {
    deserializer.deserialize_str(Text)
}

struct OptionalText;

impl<'de> Visitor<'de> for OptionalText {
    type Value = Option<String>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded metadata text or null")
    }

    fn visit_none<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_some<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        text(deserializer).map(Some)
    }
}

pub(crate) fn optional_text<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error> {
    deserializer.deserialize_option(OptionalText)
}

struct Sequence<T, const LIMIT: usize>(PhantomData<T>);

impl<'de, T: Deserialize<'de>, const LIMIT: usize> Visitor<'de> for Sequence<T, LIMIT> {
    type Value = Vec<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a sequence of at most {LIMIT} items")
    }

    fn visit_seq<A: SeqAccess<'de>>(
        self,
        mut sequence: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or_default().min(LIMIT));
        while values.len() < LIMIT {
            let Some(value) = sequence.next_element()? else {
                return Ok(values);
            };
            values.push(value);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(A::Error::invalid_length(LIMIT.saturating_add(1), &self));
        }
        Ok(values)
    }
}

pub(crate) fn sequence<'de, D, T, const LIMIT: usize>(
    deserializer: D,
) -> std::result::Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserializer.deserialize_seq(Sequence::<T, LIMIT>(PhantomData))
}

pub(crate) fn expected<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<Expected>, D::Error> {
    sequence::<D, Expected, MAX_LOGICAL_SOURCES>(deserializer)
}

pub(crate) fn artifacts<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<Artifact>, D::Error> {
    sequence::<D, Artifact, MAX_ARTIFACTS>(deserializer)
}

pub(crate) fn targets<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<Location>, D::Error> {
    sequence::<D, Location, MAX_ARTIFACTS>(deserializer)
}

pub(crate) fn findings<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<Finding>, D::Error> {
    sequence::<D, Finding, MAX_DETAILS>(deserializer)
}

struct BoundedText(String);

impl<'de> Deserialize<'de> for BoundedText {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        text(deserializer).map(Self)
    }
}

pub(crate) fn details<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<String>, D::Error> {
    sequence::<D, BoundedText, MAX_DETAILS>(deserializer)
        .map(|values| values.into_iter().map(|value| value.0).collect())
}

#[cfg(test)]
mod tests {
    use super::{MAX_PATH_BYTES, json_tokens, sequence, text};

    #[test]
    fn oversized_raw_tokens_are_rejected_before_json_string_scratch_is_allocated() {
        let bytes = format!("\"{}\"", "\\u0061".repeat(MAX_PATH_BYTES + 1));
        assert!(
            json_tokens(bytes.as_bytes()).is_err(),
            "escaped token scratch has a fixed upper bound"
        );
    }

    #[test]
    fn decoded_text_bounds_apply_before_an_owned_field_is_created()
    -> Result<(), Box<dyn std::error::Error>> {
        let encoded = format!("\"{}\"", "\\u0061".repeat(MAX_PATH_BYTES));
        json_tokens(encoded.as_bytes())?;
        let value = text(&mut serde_json::Deserializer::from_str(&encoded))?;
        assert_eq!(
            value.len(),
            MAX_PATH_BYTES,
            "valid escaped boundary text is preserved"
        );
        let oversized = format!("\"{}\"", "x".repeat(MAX_PATH_BYTES + 1));
        assert!(
            text(&mut serde_json::Deserializer::from_str(&oversized)).is_err(),
            "oversized borrowed text is rejected before copying into its field"
        );
        Ok(())
    }

    #[test]
    fn bounded_sequence_does_not_deserialize_an_extra_owned_element() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static DECODED: AtomicUsize = AtomicUsize::new(0);
        struct Counted;
        impl<'de> serde::Deserialize<'de> for Counted {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let _value = <u8 as serde::Deserialize>::deserialize(deserializer)?;
                DECODED.fetch_add(1, Ordering::SeqCst);
                Ok(Self)
            }
        }
        let mut input = serde_json::Deserializer::from_str("[1,2,3]");
        let result = sequence::<_, Counted, 2>(&mut input);
        assert!(result.is_err(), "an extra item is refused");
        assert_eq!(
            DECODED.load(Ordering::SeqCst),
            2,
            "the extra owned item was never constructed"
        );
    }
}
