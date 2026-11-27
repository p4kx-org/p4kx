// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use thiserror::Error;

use crate::{FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath, FieldPathError, Filter};

use super::{FieldStatisticsReport, FieldStatisticsRowProof};

/// A displayed statistics value cannot be bound to an exact output address.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FieldStatisticsAddressError {
    /// The output path cannot be represented.
    #[error(transparent)]
    Path(#[from] FieldPathError),
    /// An exact output path received more than one origin.
    #[error(transparent)]
    Duplicate(#[from] FieldOriginsError),
    /// The original fully reduced statistic has no corresponding row proof.
    #[error("field statistics output has no original row proof")]
    MissingRow,
}

/// Origins addressed by the display filter's actual output order.
#[derive(Debug)]
#[non_exhaustive]
pub struct FieldStatisticsAddressedOrigins {
    /// Original statistic ordinal of each emitted field, in display order.
    pub ordinals: Vec<usize>,
    /// Immediate origins at the existing primitive data value addresses.
    pub fields: FieldOrigins,
}

impl FieldStatisticsReport<'_> {
    /// Addresses original metric proofs in the display filter's row order.
    ///
    /// The shared Record selection proof applies to every row in this result.
    ///
    /// # Errors
    ///
    /// Returns an error if the output sequence or address cannot be represented.
    pub fn addressed_origins(
        &self,
        filter: Option<&Filter>,
    ) -> Result<FieldStatisticsAddressedOrigins, FieldStatisticsAddressError> {
        let mut fields = FieldOrigins::new();
        let mut ordinals = Vec::new();
        for (ordinal, field) in self.reduction.statistics().fields().iter().enumerate() {
            if filter.is_some_and(|filter| !filter.matches(field.path())) {
                continue;
            }
            let proof = self
                .fields
                .get(ordinal)
                .filter(|proof| proof.ordinal == ordinal)
                .ok_or(FieldStatisticsAddressError::MissingRow)?;
            let mut path = FieldPath::parse("/fields")?;
            path.push_index(ordinals.len())?;
            row(&mut fields, &mut path, proof, field.kinds().len())?;
            ordinals.push(ordinal);
        }
        for (name, origin) in [
            ("/records", &self.counters.records),
            ("/unreadable_records", &self.counters.unreadable_records),
            ("/unreadable_values", &self.counters.unreadable_values),
            ("/field_count", &self.counters.field_count),
        ] {
            fields.insert(FieldPath::parse(name)?, origin.clone())?;
        }
        Ok(FieldStatisticsAddressedOrigins { ordinals, fields })
    }
}

fn row(
    fields: &mut FieldOrigins,
    path: &mut FieldPath,
    proof: &FieldStatisticsRowProof,
    kind_count: usize,
) -> Result<(), FieldStatisticsAddressError> {
    for (name, origin) in [
        ("path", &proof.structure),
        ("occurrences", &proof.structure),
        ("records", &proof.structure),
        ("empty", &proof.empty),
        ("distinct", &proof.distinct),
        ("distinct_capped", &proof.distinct),
        ("min", &proof.extrema),
        ("max", &proof.extrema),
    ] {
        field(fields, path, name, origin)?;
    }
    field(fields, path, "kinds", &proof.structure)?;
    path.push_field("kinds")?;
    for index in 0..kind_count {
        path.push_index(index)?;
        field(fields, path, "kind", &proof.structure)?;
        field(fields, path, "count", &proof.structure)?;
        path.pop();
    }
    path.pop();
    Ok(())
}

fn field(
    fields: &mut FieldOrigins,
    path: &mut FieldPath,
    name: &str,
    origin: &FieldOrigin,
) -> Result<(), FieldStatisticsAddressError> {
    path.push_field(name)?;
    fields.insert(path.clone(), origin.clone())?;
    path.pop();
    Ok(())
}
