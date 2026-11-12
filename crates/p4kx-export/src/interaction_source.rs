// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use crate::CatalogError;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceRecordReference, SourceWeakReference};

pub(super) type InteractionList<T> = SourcedValue<Vec<SourcedValue<T>>>;
pub(super) type RecordLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;
pub(super) type WeakLink<'source> = SourcedValue<Box<SourceWeakReference<'source>>>;
pub(super) type RecordLinks<'source> = SourcedValue<Vec<RecordLink<'source>>>;
pub(super) type WeakLinks<'source> = SourcedValue<Vec<WeakLink<'source>>>;

/// Retains every member of one concrete authored variant.
pub(super) fn original<'source>(
    source: Observed<'_, '_, '_, 'source>,
    _notes: &mut Vec<FieldError>,
) -> Result<UnitSource<'source>, CatalogError> {
    UnitSource::read(source, [])
}
