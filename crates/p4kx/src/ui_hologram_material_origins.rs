// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::{AttachedModel, Part};
use p4kx_extraction::{
    InputScope, MaterialArchiveOrigins, SourceInput, SourceLocation, SourceScope, SourceUse,
    SourceUseRole,
};

use super::{Projection, cdf_origins, gear_origins};

pub(super) struct MaterialInputs<'a> {
    pub(super) archive: &'a MaterialArchiveOrigins,
    pub(super) owner: Option<&'a Part>,
    pub(super) attachment: Option<(usize, &'a AttachedModel)>,
}

impl MaterialInputs<'_> {
    pub(super) fn scope(
        &self,
        projection: &Projection,
        sources: &mut Vec<SourceUse>,
    ) -> Result<Option<SourceScope>> {
        let Some(origin) = self.archive.library() else {
            return Ok(None);
        };
        if let Some(part) = self.owner {
            projection.authored_selector(part.material_selector(), sources)?;
            cdf_origins::model_material(projection, part, sources)?;
        }
        if let Some((_, attachment)) = self.attachment {
            cdf_origins::material(projection, attachment, sources)?;
        }
        Ok(Some(projection.uses().observe(
            SourceInput::Archive(origin),
            InputScope::Payload,
        )?))
    }

    pub(super) fn whole_piece(
        &self,
        projection: &Projection,
        sources: &mut Vec<SourceUse>,
    ) -> Result<()> {
        if let Some(part) = self.owner {
            cdf_origins::model(projection, part, sources)?;
            gear_origins::model(projection, part, sources)?;
        }
        if let Some((_, attachment)) = self.attachment {
            cdf_origins::whole_piece_selection(projection, attachment, sources)?;
        }
        Ok(())
    }

    pub(super) fn selected(
        &self,
        projection: &Projection,
        slots: impl Iterator<Item = usize>,
        sources: &mut Vec<SourceUse>,
    ) -> Result<()> {
        let mut slots = slots.peekable();
        if slots.peek().is_none() {
            return Ok(());
        }
        if let Some(scope) = self.scope(projection, sources)? {
            for slot in slots {
                sources.push(material_slot(&scope, slot, "selectedMaterial")?);
            }
        }
        Ok(())
    }
}

pub(super) fn material_slot(scope: &SourceScope, slot: usize, field: &str) -> Result<SourceUse> {
    Ok(scope.at(
        SourceLocation::Subresource {
            namespace: "materialLibrary".into(),
            indices: vec![u64::try_from(slot)?],
            field: field.into(),
        },
        SourceUseRole::Selector,
    )?)
}
