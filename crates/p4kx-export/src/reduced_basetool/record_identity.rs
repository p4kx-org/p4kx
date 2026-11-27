// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use serde::Serialize;

use super::cell::Cell;
use crate::canonical_record_header::{CanonicalRecordHeader, CatalogRecordSource};
use crate::guid_text::CatalogGuid;
use crate::variant_target::VariantTarget;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RecordIdentity<'source> {
    guid: Cell<CatalogGuid>,
    record_index: Cell<&'source u32>,
    class_name: Cell<&'source str>,
    record_path: Cell<&'source str>,
}

impl<'source> From<&'source CanonicalRecordHeader<'_>> for RecordIdentity<'source> {
    fn from(header: &'source CanonicalRecordHeader<'_>) -> Self {
        Self {
            guid: Cell::from(&header.guid).map(|guid| *guid),
            record_index: Cell::from(header.record_index()),
            class_name: Cell::from(header.class_name()).map(|name| *name),
            record_path: Cell::from(header.record_path()).map(|path| *path),
        }
    }
}

impl<'source> From<&'source CatalogRecordSource> for RecordIdentity<'source> {
    fn from(header: &'source CatalogRecordSource) -> Self {
        Self {
            guid: Cell::from(&header.guid).map(|guid| CatalogGuid(*guid)),
            record_index: Cell::from(&header.record_index),
            class_name: Cell::from(&header.class_name).map(String::as_str),
            record_path: Cell::from(&header.record_path).map(String::as_str),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Variant<'source> {
    guid: Cell<CatalogGuid>,
    target_record_index: Cell<&'source u32>,
}

pub(super) fn variant(value: &SourcedValue<VariantTarget>) -> Cell<Variant<'_>> {
    Cell::from(value).map(|target| Variant {
        guid: Cell::from(&target.guid()).map(|guid| CatalogGuid(*guid)),
        target_record_index: Cell::from(target.target_record_index()),
    })
}
