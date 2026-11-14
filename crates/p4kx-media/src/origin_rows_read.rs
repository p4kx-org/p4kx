// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, FieldOriginSeed, OriginReaderLimits};
use serde::Deserialize;
use serde::de::{DeserializeSeed, Deserializer};

use crate::origin_contract::{OriginCoverage, OriginProfile, OriginScope, OriginStreamSummary};
use crate::origin_value_read;

/// One streamed origin row without original-input authority.
pub(crate) enum ReadRow {
    Occurrence {
        scope: OriginScope,
        profile: OriginProfile,
        payload_sha256: String,
    },
    Field {
        scope: OriginScope,
        field: String,
        origin: FieldOrigin,
    },
    End {
        scope: OriginScope,
        coverage: OriginCoverage,
    },
    Eof {
        streams: Vec<OriginStreamSummary>,
    },
}

#[derive(Clone, Copy)]
pub(crate) struct RowSeed(pub(crate) OriginReaderLimits);

impl<'de> DeserializeSeed<'de> for RowSeed {
    type Value = ReadRow;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<ReadRow, D::Error> {
        use serde::de::Error as _;
        let mut parts = Parts::deserialize(deserializer)?;
        let row = match parts.kind {
            RowKind::Occurrence => ReadRow::Occurrence {
                scope: take(&mut parts.scope, "scope")?,
                profile: take(&mut parts.profile, "profile")?,
                payload_sha256: take(&mut parts.payload_sha256, "payload_sha256")?,
            },
            RowKind::Field => {
                let scope = take(&mut parts.scope, "scope")?;
                let field = take(&mut parts.field, "field")?;
                if field.len() > self.0.path_bytes {
                    return Err(D::Error::custom(
                        "field path exceeds the reader byte policy",
                    ));
                }
                ReadRow::Field {
                    scope,
                    field,
                    origin: parts.origin(self.0).map_err(D::Error::custom)?,
                }
            }
            RowKind::End => ReadRow::End {
                scope: take(&mut parts.scope, "scope")?,
                coverage: take(&mut parts.coverage, "coverage")?,
            },
            RowKind::Eof => ReadRow::Eof {
                streams: take(&mut parts.streams, "streams")?,
            },
        };
        parts.require_consumed().map_err(D::Error::custom)?;
        Ok(row)
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum RowKind {
    Occurrence,
    Field,
    End,
    Eof,
}

#[derive(Default)]
enum Marker {
    #[default]
    Missing,
    Present(Option<bool>),
}

#[derive(Deserialize)]
struct Parts {
    #[serde(rename = "type")]
    kind: RowKind,
    scope: Option<OriginScope>,
    profile: Option<OriginProfile>,
    payload_sha256: Option<String>,
    field: Option<String>,
    #[serde(
        rename = "containsEncryptedSource",
        default,
        deserialize_with = "read_marker"
    )]
    marker: Marker,
    #[serde(rename = "sourceUsesRefs")]
    references: Option<Vec<String>>,
    #[serde(rename = "sourceSetComplete")]
    complete: Option<bool>,
    coverage: Option<OriginCoverage>,
    streams: Option<Vec<OriginStreamSummary>>,
}

impl Parts {
    fn origin(
        &mut self,
        limits: OriginReaderLimits,
    ) -> Result<FieldOrigin, serde::de::value::Error> {
        use serde::de::Error as _;
        let marker = match std::mem::take(&mut self.marker) {
            Marker::Present(marker) => marker,
            Marker::Missing => {
                return Err(serde::de::value::Error::missing_field(
                    "containsEncryptedSource",
                ));
            }
        };
        let references = take(&mut self.references, "sourceUsesRefs")?;
        let complete = take(&mut self.complete, "sourceSetComplete")?;
        FieldOriginSeed::new(limits).deserialize(origin_value_read::deserializer(
            marker, references, complete,
        ))
    }

    const fn require_consumed(&self) -> Result<(), &'static str> {
        if self.scope.is_some()
            || self.profile.is_some()
            || self.payload_sha256.is_some()
            || self.field.is_some()
            || !matches!(self.marker, Marker::Missing)
            || self.references.is_some()
            || self.complete.is_some()
            || self.coverage.is_some()
            || self.streams.is_some()
        {
            return Err("origin row contains fields of a different closed row variant");
        }
        Ok(())
    }
}

fn take<T, E: serde::de::Error>(field: &mut Option<T>, name: &'static str) -> Result<T, E> {
    field.take().ok_or_else(|| E::missing_field(name))
}

fn read_marker<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Marker, D::Error> {
    Option::<bool>::deserialize(deserializer).map(Marker::Present)
}

#[cfg(test)]
#[path = "origin_rows_read_tests.rs"]
mod tests;
