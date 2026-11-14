// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::{AttachedModel, CdfAttachmentField, CdfAttachmentSite, Part};
use p4kx_extraction::{
    EntryOrigin, InputScope, SourceInput, SourceLocation, SourceUse, SourceUseRole,
};

use super::{Projection, bone_origins};

pub(super) fn attachment(
    projection: &Projection,
    owner: &Part,
    ordinal: usize,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    let Some(attachment) = owner.attachments().get(ordinal) else {
        return Ok(());
    };
    let fields = attachment.cdf_sources().iter().filter(|source| {
        let field = source.field();
        field != CdfAttachmentField::Material && (!field.is_offset() || attachment.offset_used())
    });
    attachment_fields(projection, attachment, fields, uses)?;
    if let Some(bone) = owner.attachment_bone_placement_source(ordinal) {
        bone_origins::record(projection, &bone, uses)?;
    }
    Ok(())
}

pub(super) fn whole_piece_selection(
    projection: &Projection,
    attachment: &AttachedModel,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    let fields = attachment.cdf_sources().iter().filter(|source| {
        matches!(
            source.field(),
            CdfAttachmentField::Type | CdfAttachmentField::Binding
        )
    });
    attachment_fields(projection, attachment, fields, uses)
}

pub(super) fn material(
    projection: &Projection,
    attachment: &AttachedModel,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    let fields = attachment
        .cdf_sources()
        .iter()
        .filter(|source| source.field() == CdfAttachmentField::Material);
    attachment_fields(projection, attachment, fields, uses)
}

pub(super) fn model(projection: &Projection, part: &Part, uses: &mut Vec<SourceUse>) -> Result<()> {
    model_selectors(
        projection,
        part.model_definition_origin(),
        part.cdf_model_selectors(),
        uses,
    )
}

pub(super) fn model_material(
    projection: &Projection,
    part: &Part,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    model_selectors(
        projection,
        part.model_definition_origin(),
        part.cdf_material_selector().into_iter(),
        uses,
    )
}

pub(super) fn model_selectors(
    projection: &Projection,
    origin: Option<&EntryOrigin>,
    fields: impl Iterator<Item = SourceLocation>,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    record(
        projection,
        origin,
        fields.map(|location| (location, SourceUseRole::Selector)),
        uses,
    )
}

fn attachment_fields<'a>(
    projection: &Projection,
    attachment: &AttachedModel,
    fields: impl Iterator<Item = &'a CdfAttachmentSite>,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    let fields = fields.filter_map(|source| {
        let role = if source.field().is_offset() {
            SourceUseRole::Conversion
        } else {
            SourceUseRole::Selector
        };
        Some((source.location()?, role))
    });
    record(
        projection,
        attachment.model_definition_origin(),
        fields,
        uses,
    )
}

fn record(
    projection: &Projection,
    origin: Option<&EntryOrigin>,
    fields: impl Iterator<Item = (SourceLocation, SourceUseRole)>,
    uses: &mut Vec<SourceUse>,
) -> Result<()> {
    let Some(origin) = origin else {
        return Ok(());
    };
    let mut fields = fields.peekable();
    if fields.peek().is_none() {
        return Ok(());
    }
    let scope = projection
        .uses()
        .observe(SourceInput::Archive(origin), InputScope::Payload)?;
    for (location, role) in fields {
        uses.push(scope.at(location, role)?);
    }
    Ok(())
}
