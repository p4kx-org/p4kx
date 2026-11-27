// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

/// A selected physical group and its separately owned parent projection.
#[derive(Debug)]
pub(crate) struct ResourceGroup<'source> {
    pub(crate) guid: CatalogGuid,
    pub(crate) record_path: &'source str,
    pub(crate) record_index: u32,
    pub(crate) definition: ResourceGroupDefinition<'source>,
    pub(crate) parent: ResourceGroupParent,
}

/// Direct scalar facts of one physical `ResourceTypeGroup` Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ResourceGroupDefinition<'source> {
    #[serde(flatten)]
    pub(crate) source: CanonicalRecordHeader<'source>,
    pub(crate) name: SourcedValue<PublicNameRef>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: SourcedValue<GameTextRef>,
    pub(crate) default_thumbnail_path: SourcedValue<&'source str>,
    #[serde(rename = "defaultThumbnailPathSVG")]
    pub(crate) default_thumbnail_path_svg: SourcedValue<&'source str>,
}

/// The original ordered membership arrays of one selected physical group.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ResourceGroupMembers {
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) physical: ResourceGroupKey,
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) record_index: SourcedValue<u32>,
    pub(crate) groups: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
    pub(crate) resources: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
}

/// The first selected parent with a different GUID, separate from raw memberships.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ResourceGroupParent {
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) record_index: SourcedValue<u32>,
    pub(crate) parent: SourcedValue<Option<CatalogRecordReference>>,
}

/// Private physical sorting identity, independent of serialized field states.
#[derive(Debug)]
pub(crate) struct ResourceGroupKey {
    pub(crate) guid: CatalogGuid,
    pub(crate) record_index: u32,
}

impl<'source> ResourceGroupDefinition<'source> {
    /// Borrows the original physical owner header.
    #[must_use]
    pub const fn header(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// Borrows the public localized group name identity.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<PublicNameRef> {
        &self.name
    }

    /// Borrows the protected localized description identity.
    #[must_use]
    pub const fn description(&self) -> &SourcedValue<GameTextRef> {
        &self.description
    }

    /// Borrows the original raster thumbnail path.
    #[must_use]
    pub const fn default_thumbnail_path(&self) -> &SourcedValue<&'source str> {
        &self.default_thumbnail_path
    }

    /// Borrows the original vector thumbnail path.
    #[must_use]
    pub const fn default_thumbnail_path_svg(&self) -> &SourcedValue<&'source str> {
        &self.default_thumbnail_path_svg
    }
}

impl ResourceGroupMembers {
    /// Borrows the original external owner GUID.
    #[must_use]
    pub const fn guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.guid
    }

    /// Borrows the exact physical owner index.
    #[must_use]
    pub const fn record_index(&self) -> &SourcedValue<u32> {
        &self.record_index
    }

    /// Borrows original child-group slots in authored order.
    #[must_use]
    pub const fn groups(&self) -> &SourcedValue<Vec<SourcedValue<CatalogRecordReference>>> {
        &self.groups
    }

    /// Borrows original resource slots in authored order.
    #[must_use]
    pub const fn resources(&self) -> &SourcedValue<Vec<SourcedValue<CatalogRecordReference>>> {
        &self.resources
    }
}

impl ResourceGroupParent {
    /// Borrows the original external child GUID.
    #[must_use]
    pub const fn guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.guid
    }

    /// Borrows the exact physical child index.
    #[must_use]
    pub const fn record_index(&self) -> &SourcedValue<u32> {
        &self.record_index
    }

    /// Borrows the canonical first-parent choice and its source knowledge.
    #[must_use]
    pub const fn parent(&self) -> &SourcedValue<Option<CatalogRecordReference>> {
        &self.parent
    }
}
