// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourcedValue};

use super::{ItemAttachment, ItemAttachmentComponent};
use crate::catalog_v3::authored_fields::AuthoredRestriction;
use crate::catalog_v3::item_alignment_options::AttachmentAxisOptionStatus;
use crate::resource_number::ResourceNumber;

impl<'source> ItemAttachmentComponent<'source> {
    /// Returns the nominal owner GUID with its original header origin.
    #[must_use]
    pub fn owner_guid(&self) -> SourcedValue<Guid> {
        self.owner.guid().map_ref(|guid| guid.0)
    }

    /// Borrows the physical Record index in the queried DataForge.
    #[must_use]
    pub const fn owner_record_index(&self) -> &SourcedValue<u32> {
        self.owner.record_index()
    }

    /// Borrows the original Components array ordinal.
    #[must_use]
    pub const fn component_ordinal(&self) -> &SourcedValue<u32> {
        self.owner.component_ordinal()
    }

    /// Borrows the original attachment definition and its retained state.
    #[must_use]
    pub const fn definition(&self) -> &SourcedValue<ItemAttachment<'source>> {
        &self.definition
    }
}

impl<'source> ItemAttachment<'source> {
    /// Borrows the declared item type or its original storage shape.
    #[must_use]
    pub const fn item_type(&self) -> &SourcedValue<AuthoredRestriction<'source, &'source str>> {
        &self.item_type
    }

    /// Borrows the declared item subtype or its original storage shape.
    #[must_use]
    pub const fn item_subtype(&self) -> &SourcedValue<AuthoredRestriction<'source, &'source str>> {
        &self.item_subtype
    }

    /// Borrows the declared item size or its original storage shape.
    #[must_use]
    pub const fn size(&self) -> &SourcedValue<AuthoredRestriction<'source, ResourceNumber>> {
        &self.size
    }

    /// Borrows the original item tags without token normalization.
    #[must_use]
    pub const fn tags(&self) -> &SourcedValue<&'source str> {
        &self.tags
    }

    /// Borrows the original tags required for attachment.
    #[must_use]
    pub const fn required_tags(&self) -> &SourcedValue<&'source str> {
        &self.required_tags
    }

    /// Borrows the authored attachment alignment axis label.
    #[must_use]
    pub const fn alignment_axis(&self) -> &SourcedValue<&'source str> {
        &self.attach_axes_to_align.label
    }

    /// Borrows the alignment enum-membership result when the authored label is readable.
    #[must_use]
    pub const fn alignment_axis_option_status(
        &self,
    ) -> Option<&SourcedValue<AttachmentAxisOptionStatus>> {
        self.attach_axes_to_align.option_status.as_ref()
    }

    /// Borrows the authored axis ignored during alignment.
    #[must_use]
    pub const fn ignored_alignment_axis(&self) -> &SourcedValue<&'source str> {
        &self.ignored_attach_axis.label
    }

    /// Borrows the ignored-axis enum-membership result when its authored label is readable.
    #[must_use]
    pub const fn ignored_alignment_axis_option_status(
        &self,
    ) -> Option<&SourcedValue<AttachmentAxisOptionStatus>> {
        self.ignored_attach_axis.option_status.as_ref()
    }

    /// Borrows the original centre-of-mass attachment check.
    #[must_use]
    pub const fn attach_check_center_of_mass(&self) -> &SourcedValue<bool> {
        &self.attach_check_center_of_mass
    }
}
