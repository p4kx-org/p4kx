// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocGeometryTarget, resolve_soc_geometry_path};
use p4kx_extraction::model::{SocPhysicsField, SocSourceValue};
use p4kx_extraction::{
    CancelToken, EntryOrigin, InputScope, PathIndex, SourceInput, SourceUse, SourceUses,
    SourceValue, SourcedValue, directory_path_uses,
};

use super::LocationVoxelTarget;
use crate::catalog_read::original::{EntrySourceXml, PhysicalEntryRef};
pub(super) use crate::container_read::observe_container_directory;
use crate::{CatalogError, ExportAccountingError};

pub(super) struct Directories<'index, 'archive> {
    pub(super) paths: &'index [PathIndex<'archive>],
    pub(super) local_proof: Option<SourceUse>,
    pub(super) root_proof: Option<&'index SourceUse>,
    pub(super) cancel: Option<&'index CancelToken>,
}

impl Directories<'_, '_> {
    pub(super) fn voxel(
        &self,
        xml: &EntrySourceXml<'_>,
        field: &SocPhysicsField<'_, &str>,
    ) -> Result<SourcedValue<LocationVoxelTarget>, CatalogError> {
        let mut uses = xml.physics_selector(field)?;
        let result = match (*field).into_value() {
            SocSourceValue::Absent => {
                return Ok(xml.sources().value(SourceValue::Absent, &uses, true));
            }
            SocSourceValue::Present(path) => {
                resolve_soc_geometry_path(path, self.paths, self.cancel)?
            }
            _ => SocGeometryTarget::InvalidSelector,
        };
        let local = matches!(
            result,
            SocGeometryTarget::Matched { tier, .. } | SocGeometryTarget::Ambiguous { tier, .. }
                if tier.depth == 0
        );
        let invalid = matches!(result, SocGeometryTarget::InvalidSelector);
        let complete =
            invalid || (self.local_proof.is_some() && (local || self.root_proof.is_some()));
        let state = if !complete && matches!(result, SocGeometryTarget::Missing) {
            LocationVoxelTarget::Unavailable
        } else {
            target(result, xml.sources(), &mut uses)?
        };
        if !invalid {
            uses.extend(self.local_proof.clone());
            if !local {
                uses.extend(self.root_proof.cloned());
            }
        }
        Ok(xml
            .sources()
            .value(SourceValue::Present(state), &uses, complete))
    }
}

fn target(
    result: SocGeometryTarget,
    sources: &SourceUses,
    uses: &mut Vec<SourceUse>,
) -> Result<LocationVoxelTarget, CatalogError> {
    match result {
        SocGeometryTarget::InvalidSelector => Ok(LocationVoxelTarget::Invalid),
        SocGeometryTarget::Missing => Ok(LocationVoxelTarget::Missing),
        SocGeometryTarget::Matched { entry, .. } => Ok(LocationVoxelTarget::Matched {
            entry: physical(&entry, sources, uses)?,
        }),
        SocGeometryTarget::Ambiguous { entries, .. } => Ok(LocationVoxelTarget::Ambiguous {
            candidates: entries
                .iter()
                .map(|entry| physical(entry, sources, uses))
                .collect::<Result<_, _>>()?,
        }),
        _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
}

fn physical(
    entry: &EntryOrigin,
    sources: &SourceUses,
    uses: &mut Vec<SourceUse>,
) -> Result<PhysicalEntryRef, CatalogError> {
    let scope = sources.observe(
        SourceInput::Archive(entry),
        InputScope::ArchiveDirectoryMetadata,
    )?;
    let (path, _) = directory_path_uses(&scope, entry.ancestors().len(), true)?;
    uses.extend(path);
    Ok(PhysicalEntryRef {
        input_ref: scope.source_ref().clone(),
        entry_index: entry.binding().entry_index(),
    })
}
