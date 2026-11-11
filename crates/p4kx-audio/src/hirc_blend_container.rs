// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_rtpc::{self, BankRtpcExtent, CURVE_POINT_LEN};

/// Original fields following the children of a version-154 Blend container.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankBlendContainerFields {
    /// Start of the original Layer continuation.
    pub start: u64,
    /// Original Layer table count and checked prefix when reached.
    pub layers: Option<BankBlendExtent>,
    /// Original continuous-validation byte, including unknown bits.
    pub continuous_validation: Option<BankHircField<u8>>,
}

impl BankBlendContainerFields {
    pub(crate) const fn new(start: u64) -> Self {
        Self {
            start,
            layers: None,
            continuous_validation: None,
        }
    }

    /// Returns the extent when all declared Layers and the final byte fit.
    #[must_use]
    pub fn complete_range(&self) -> Option<Range<u64>> {
        self.continuous_validation
            .as_ref()
            .map(|word| self.start..word.range.end)
    }
}

/// One Blend table's original count and checked physical extent.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankBlendExtent {
    /// Original authored count.
    pub declared_count: BankHircField<u32>,
    /// Checked original rows, including any incomplete last field.
    pub checked_range: Range<u64>,
    complete: bool,
}

impl BankBlendExtent {
    /// Returns whether all declared table rows fit the owning object.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }
}

/// Original data encountered inside one physical Layer.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankBlendLayerFields {
    /// Original Layer ID.
    pub layer_id: BankHircField<u32>,
    /// Checked original initial RTPC data when its count fits.
    pub rtpc: Option<BankRtpcExtent>,
    /// Original crossfade RTPC ID, including zero.
    pub crossfade_rtpc_id: Option<BankHircField<u32>>,
    /// Original crossfade RTPC type code.
    pub crossfade_rtpc_type: Option<BankHircField<u8>>,
    /// Original association count and checked prefix when reached.
    pub associations: Option<BankBlendExtent>,
}

impl BankBlendLayerFields {
    const fn new(layer_id: BankHircField<u32>) -> Self {
        Self {
            layer_id,
            rtpc: None,
            crossfade_rtpc_id: None,
            crossfade_rtpc_type: None,
            associations: None,
        }
    }
}

/// Original fields of one child association within a Layer.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankBlendAssociationFields {
    /// Original associated child reference.
    pub child_id: BankHircField<u32>,
    /// Original point count and checked graph point extent.
    pub points: Option<BankBlendExtent>,
}

pub(crate) fn tail<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankBlendContainerFields,
) -> Result<(), F::Error> {
    scan_table(input, &mut observed.layers, |fields, count| {
        for _ in 0..count {
            let id = fields.read_u32()?;
            scan_layer(fields, id).1?;
        }
        Ok(())
    })?;
    observed.continuous_validation = Some(input.read_u8()?);
    Ok(())
}

pub(crate) fn scan_layer<F: ObjectFields>(
    input: &mut F,
    layer_id: BankHircField<u32>,
) -> (BankBlendLayerFields, Result<(), F::Error>) {
    let mut fields = BankBlendLayerFields::new(layer_id);
    let read = layer(input, &mut fields);
    (fields, read)
}

fn layer<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankBlendLayerFields,
) -> Result<(), F::Error> {
    hirc_rtpc::scan(input, &mut observed.rtpc)?;
    observed.crossfade_rtpc_id = Some(input.read_u32()?);
    observed.crossfade_rtpc_type = Some(input.read_u8()?);
    scan_table(input, &mut observed.associations, |fields, count| {
        for _ in 0..count {
            let child_id = fields.read_u32()?;
            scan_association(fields, child_id).1?;
        }
        Ok(())
    })
}

pub(crate) fn scan_association<F: ObjectFields>(
    input: &mut F,
    child_id: BankHircField<u32>,
) -> (BankBlendAssociationFields, Result<(), F::Error>) {
    let mut fields = BankBlendAssociationFields {
        child_id,
        points: None,
    };
    let read = scan_table(input, &mut fields.points, |fields, count| {
        fields.skip_bytes(u64::from(count) * CURVE_POINT_LEN as u64)
    });
    (fields, read)
}

fn scan_table<F: ObjectFields>(
    input: &mut F,
    observed: &mut Option<BankBlendExtent>,
    scan: impl FnOnce(&mut F, u32) -> Result<(), F::Error>,
) -> Result<(), F::Error> {
    let declared_count = input.read_u32()?;
    let start = input.offset();
    let read = scan(input, declared_count.value);
    let complete = read.is_ok();
    let end = if complete {
        input.offset()
    } else {
        input.offset() + input.remaining_bytes()
    };
    *observed = Some(BankBlendExtent {
        declared_count,
        checked_range: start..end,
        complete,
    });
    read
}
