// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_graph_point_rows::BankGraphPointRows;
use crate::bank_rtpc_rows::BankRtpcRows;
use crate::bank_table_rows::BankTableRows;
use crate::hirc_blend_container::{
    self, BankBlendAssociationFields, BankBlendExtent, BankBlendLayerFields,
};
use crate::read_at::SeekSource;
use crate::{BankError, BankReadError};

/// Sequential cursor over the checked physical Blend Layers.
pub struct BankBlendLayers<'input, R>(BankTableRows<'input, R>);

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
        Self(BankTableRows::new(
            source,
            stopped,
            &extent.checked_range,
            extent.declared_count.value,
        ))
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
        Some(BankBlendAssociations(BankTableRows::new(
            self.source,
            self.stopped,
            &extent.checked_range,
            extent.declared_count.value,
        )))
    }
}

/// Sequential cursor over the checked child associations within one Layer.
pub struct BankBlendAssociations<'input, R>(BankTableRows<'input, R>);

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
    pub fn points(&mut self) -> Option<BankGraphPointRows<'_, R>> {
        let extent = self.fields.points.as_ref()?;
        Some(BankGraphPointRows::new(
            self.source,
            self.stopped,
            &extent.checked_range,
            extent.declared_count.value,
        ))
    }
}
