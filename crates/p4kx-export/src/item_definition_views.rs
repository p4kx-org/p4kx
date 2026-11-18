// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;

use super::{
    ItemAttachmentPresentation, ItemDefinition, ItemDefinitionComponent,
    ItemDefinitionComponentKind, ItemLocalization, ItemPhysics, ItemPurchasable,
    WeaponAttachmentMotion,
};
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::catalog_v3::authored_fields::authored_manufacturer::AuthoredManufacturer;
use crate::resource_float::ResourceFloat32;
use crate::resource_number::ResourceNumber;

impl<'source> ItemDefinition<'source> {
    /// Borrows the independently sourced original Record header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.header
    }

    /// Borrows the retained original components in their authored order.
    #[must_use]
    pub const fn components(&self) -> &SourcedValue<Vec<ItemDefinitionComponent<'source>>> {
        &self.components
    }
}

impl<'source> ItemDefinitionComponent<'source> {
    /// Borrows the original Components array position.
    #[must_use]
    pub const fn component_ordinal(&self) -> &SourcedValue<u32> {
        &self.component_ordinal
    }

    /// Borrows the original concrete component type and its selector evidence.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the typed original presentation payload.
    #[must_use]
    pub const fn body(&self) -> &ItemDefinitionComponentKind<'source> {
        &self.body
    }
}

impl<'source> ItemAttachmentPresentation<'source> {
    /// Borrows the original grade without narrowing its numeric type.
    #[must_use]
    pub const fn grade(&self) -> &SourcedValue<ResourceNumber> {
        &self.grade
    }

    /// Borrows the original manufacturer Reference or authored storage.
    #[must_use]
    pub const fn manufacturer(&self) -> &AuthoredManufacturer<'source> {
        &self.manufacturer
    }

    /// Borrows the original localization declaration and its retained state.
    #[must_use]
    pub const fn localization(&self) -> &SourcedValue<ItemLocalization> {
        &self.localization
    }

    /// Borrows retained personal-weapon inputs when the authored type selects them.
    #[must_use]
    pub const fn weapon_motion(&self) -> Option<&WeaponAttachmentMotion<'source>> {
        self.weapon_motion.as_ref()
    }
}

impl ItemLocalization {
    /// Borrows the original public-name reference.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<PublicNameRef> {
        &self.name
    }

    /// Borrows the original protected-description reference.
    #[must_use]
    pub const fn description(&self) -> &SourcedValue<GameTextRef> {
        &self.description
    }
}

impl ItemPhysics {
    /// Borrows the original mass in kilograms with exact Single bits.
    #[must_use]
    pub const fn mass(&self) -> &SourcedValue<ResourceFloat32> {
        &self.mass
    }
}

impl ItemPurchasable {
    /// Borrows the original public display-name reference.
    #[must_use]
    pub const fn display_name(&self) -> &SourcedValue<PublicNameRef> {
        &self.display_name
    }

    /// Borrows the original protected display-type reference.
    #[must_use]
    pub const fn display_type(&self) -> &SourcedValue<GameTextRef> {
        &self.display_type
    }
}

impl<'source> WeaponAttachmentMotion<'source> {
    /// Borrows the original personal-weapon type with its source storage.
    #[must_use]
    pub const fn item_type(&self) -> &SourcedValue<&'source str> {
        &self.item_type
    }

    /// Borrows the original attachment tags without tokenization.
    #[must_use]
    pub const fn tags(&self) -> &SourcedValue<&'source str> {
        &self.tags
    }
}
