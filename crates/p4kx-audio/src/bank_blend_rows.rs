// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_blend_point_rows::BankBlendPointRows;
use crate::bank_object_fields::{BankHircField, ObjectFields, SeekObjectFields};
use crate::bank_rtpc_rows::BankRtpcRows;
use crate::hirc_blend_container::{
    self, BankBlendAssociationFields, BankBlendExtent, BankBlendLayerFields,
};
use crate::read_at::SeekSource;
use crate::{BankError, BankReadError};

const ID_LEN: u64 = size_of::<u32>() as u64;

/// Sequential cursor over the checked physical Blend Layers.
pub struct BankBlendLayers<'input, R>(TableRows<'input, R>);

/// One original Layer with its owner-bound nested cursors.
#[non_exhaustive]
pub struct BankBlendLayer<'input, R> {
    /// Zero-based ordinal in the authored Layer list.
    pub ordinal: u32,
    /// Successfully read Layer fields and extents.
    pub fields: BankBlendLayerFields,
    /// Checked original Layer bytes including a partial final field.
    pub checked_range: Range<u64>,
    /// Format error when the original Layer is incomplete.
    pub issue: Option<BankError>,
    source: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
}

impl<'input, R> BankBlendLayers<'input, R> {
    pub(crate) const fn new(
        source: &'input mut SeekSource<R>,
        stopped: &'input mut bool,
        extent: &BankBlendExtent,
    ) -> Self {
        Self(TableRows::new(source, stopped, extent))
    }
}

impl<R: Read + Seek> BankBlendLayers<'_, R> {
    /// Reads the next physical Layer whose original ID fits.
    ///
    /// # Errors
    ///
    /// Returns an error if checked source bytes cannot be replayed.
    pub fn next_layer(&mut self) -> Result<Option<BankBlendLayer<'_, R>>, BankReadError> {
        let Some(row) = self
            .0
            .read(|input, id| hirc_blend_container::scan_layer(input, id))?
        else {
            return Ok(None);
        };
        Ok(Some(BankBlendLayer {
            ordinal: row.ordinal,
            fields: row.fields,
            checked_range: row.checked_range,
            issue: row.issue,
            source: self.0.source,
            stopped: self.0.stopped,
        }))
    }
}

impl<R> BankBlendLayer<'_, R> {
    /// Borrows the original RTPC count and checked curve prefix.
    pub fn rtpc(&mut self) -> Option<BankRtpcRows<'_, R>> {
        let extent = self.fields.rtpc.as_ref()?;
        Some(BankRtpcRows::new(self.source, extent))
    }

    /// Borrows the checked original child associations.
    pub fn associations(&mut self) -> Option<BankBlendAssociations<'_, R>> {
        let extent = self.fields.associations.as_ref()?;
        Some(BankBlendAssociations(TableRows::new(
            self.source,
            self.stopped,
            extent,
        )))
    }
}

/// Sequential cursor over the checked child associations within one Layer.
pub struct BankBlendAssociations<'input, R>(TableRows<'input, R>);

/// One original child association and its owner-bound point cursor.
#[non_exhaustive]
pub struct BankBlendAssociation<'input, R> {
    /// Zero-based ordinal in the authored association list.
    pub ordinal: u32,
    /// Successfully read association fields and extents.
    pub fields: BankBlendAssociationFields,
    /// Checked original association bytes including a partial final field.
    pub checked_range: Range<u64>,
    /// Format error when the original association is incomplete.
    pub issue: Option<BankError>,
    source: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
}

impl<R: Read + Seek> BankBlendAssociations<'_, R> {
    /// Reads the next original association whose child ID fits.
    ///
    /// # Errors
    ///
    /// Returns an error if checked source bytes cannot be replayed.
    pub fn next_association(
        &mut self,
    ) -> Result<Option<BankBlendAssociation<'_, R>>, BankReadError> {
        let Some(row) = self
            .0
            .read(|input, id| hirc_blend_container::scan_association(input, id))?
        else {
            return Ok(None);
        };
        Ok(Some(BankBlendAssociation {
            ordinal: row.ordinal,
            fields: row.fields,
            checked_range: row.checked_range,
            issue: row.issue,
            source: self.0.source,
            stopped: self.0.stopped,
        }))
    }
}

impl<R> BankBlendAssociation<'_, R> {
    /// Borrows the checked original graph points in source order.
    pub fn points(&mut self) -> Option<BankBlendPointRows<'_, R>> {
        let extent = self.fields.points.as_ref()?;
        Some(BankBlendPointRows::new(self.source, self.stopped, extent))
    }
}

struct TableRows<'input, R> {
    source: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    range: Range<u64>,
    ordinals: Range<u32>,
}

struct Row<T> {
    ordinal: u32,
    fields: T,
    checked_range: Range<u64>,
    issue: Option<BankError>,
}

impl<'input, R> TableRows<'input, R> {
    const fn new(
        source: &'input mut SeekSource<R>,
        stopped: &'input mut bool,
        extent: &BankBlendExtent,
    ) -> Self {
        let range = &extent.checked_range;
        Self {
            source,
            stopped,
            range: range.start..range.end,
            ordinals: 0..extent.declared_count.value,
        }
    }
}

impl<R: Read + Seek> TableRows<'_, R> {
    fn read<T>(
        &mut self,
        scan: impl FnOnce(
            &mut SeekObjectFields<'_, R>,
            BankHircField<u32>,
        ) -> (T, Result<(), BankReadError>),
    ) -> Result<Option<Row<T>>, BankReadError> {
        let start = self.range.start;
        let available = self.range.end - start;
        if *self.stopped || available < ID_LEN {
            return Ok(None);
        }
        let Some(ordinal) = self.ordinals.next() else {
            return Ok(None);
        };
        let mut input = SeekObjectFields::after_id(self.source, start, available, 0);
        let id = input.read_u32().inspect_err(|_| *self.stopped = true)?;
        let (fields, result) = scan(&mut input, id);
        let (end, issue) = match result {
            Ok(()) => (input.offset(), None),
            Err(BankReadError::Format(error)) => (self.range.end, Some(error)),
            Err(error) => {
                *self.stopped = true;
                return Err(error);
            }
        };
        self.range.start = end;
        Ok(Some(Row {
            ordinal,
            fields,
            checked_range: start..end,
            issue,
        }))
    }
}
