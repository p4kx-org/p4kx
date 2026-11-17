// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::display::ManufacturerDisplayFeatures;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

/// One physical Manufacturer definition with directly sourced authored facts.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ManufacturerDefinition<'source> {
    #[serde(flatten)]
    pub(crate) source: CanonicalRecordHeader<'source>,
    pub(crate) code: SourcedValue<&'source str>,
    pub(crate) localization: SourcedValue<ManufacturerLocalization<'source>>,
    pub(crate) audio_manufacturer_tag: SourcedValue<CatalogRecordReference>,
    pub(crate) building_blocks_style: SourcedValue<CatalogRecordReference>,
    pub(crate) dashboard_canvas_config: SourcedValue<CatalogRecordReference>,
    pub(crate) light_amplification: SourcedValue<CatalogRecordReference>,
    pub(crate) logo: SourcedValue<&'source str>,
    pub(crate) logo_full_color: SourcedValue<&'source str>,
    pub(crate) logo_simplified_white: SourcedValue<&'source str>,
}

/// The Manufacturer's owned localization declaration and its canonical text links.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ManufacturerLocalization<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) name: SourcedValue<PublicNameRef>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: SourcedValue<GameTextRef>,
    pub(crate) short_name: SourcedValue<PublicNameRef>,
    pub(crate) display_features: SourcedValue<ManufacturerDisplayFeatures<'source>>,
}

impl<'source> ManufacturerDefinition<'source> {
    /// The original physical Record identity and header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// The original manufacturer code.
    #[must_use]
    pub const fn code(&self) -> &SourcedValue<&'source str> {
        &self.code
    }

    /// Owned localization fields and canonical text references.
    #[must_use]
    pub const fn localization(&self) -> &SourcedValue<ManufacturerLocalization<'source>> {
        &self.localization
    }

    /// The original manufacturer audio tag reference.
    #[must_use]
    pub const fn audio_manufacturer_tag(&self) -> &SourcedValue<CatalogRecordReference> {
        &self.audio_manufacturer_tag
    }

    /// The original Building Blocks style reference.
    #[must_use]
    pub const fn building_blocks_style(&self) -> &SourcedValue<CatalogRecordReference> {
        &self.building_blocks_style
    }

    /// The original dashboard canvas configuration reference.
    #[must_use]
    pub const fn dashboard_canvas_config(&self) -> &SourcedValue<CatalogRecordReference> {
        &self.dashboard_canvas_config
    }

    /// The original light amplification reference.
    #[must_use]
    pub const fn light_amplification(&self) -> &SourcedValue<CatalogRecordReference> {
        &self.light_amplification
    }

    /// The original logo asset path.
    #[must_use]
    pub const fn logo(&self) -> &SourcedValue<&'source str> {
        &self.logo
    }

    /// The original full-colour logo asset path.
    #[must_use]
    pub const fn logo_full_color(&self) -> &SourcedValue<&'source str> {
        &self.logo_full_color
    }

    /// The original simplified white logo asset path.
    #[must_use]
    pub const fn logo_simplified_white(&self) -> &SourcedValue<&'source str> {
        &self.logo_simplified_white
    }
}

impl<'source> ManufacturerLocalization<'source> {
    /// The original localization struct type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// The manufacturer name in the public-name domain.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<PublicNameRef> {
        &self.name
    }

    /// The manufacturer description in the protected text domain.
    #[must_use]
    pub const fn description(&self) -> &SourcedValue<GameTextRef> {
        &self.description
    }

    /// The short manufacturer name in the public-name domain.
    #[must_use]
    pub const fn short_name(&self) -> &SourcedValue<PublicNameRef> {
        &self.short_name
    }

    /// The original display feature declarations.
    #[must_use]
    pub const fn display_features(&self) -> &SourcedValue<ManufacturerDisplayFeatures<'source>> {
        &self.display_features
    }
}

/// Prepared definition and the exact keys needed for sorting and target-row accounting.
#[derive(Debug)]
pub(crate) struct Manufacturer<'source> {
    pub(crate) guid: CatalogGuid,
    pub(crate) record_path: &'source str,
    pub(crate) definition: ManufacturerDefinition<'source>,
}
