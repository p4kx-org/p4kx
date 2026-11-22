// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{FieldOrigin, Guid, SourceUses, SourceValue, SourcedValue};

use super::{LocationObject, NoObject, Objects};
use crate::catalog_v3::starmap_placement::PlacementBinding;
use crate::guid_text::CatalogGuid;
use crate::starmap_containers::proof::{PlacementEvidence, PlacementField};
use crate::{CatalogError, ExportAccountingError};

impl Objects<'_> {
    pub(crate) fn location_origin(
        &self,
        under: Option<Guid>,
        expected: Result<CatalogGuid, NoObject>,
        sources: &SourceUses,
    ) -> Result<SourcedValue<CatalogGuid>, CatalogError> {
        let mut seen = HashSet::new();
        let mut selected = Vec::new();
        let result = self.visit_location(under, |guid| {
            if seen.insert(guid) {
                selected.push(guid);
            }
        });
        if expected != result {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let mut contributions = ChainContributions {
            origins: Vec::new(),
            complete: !matches!(result, Err(NoObject::NotInCatalog)),
            ambiguous_parent: false,
        };
        for guid in selected {
            let object = self
                .by_guid
                .get(&guid)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            contributions.append(object, sources)?;
        }
        let state = match result {
            Ok(guid) => SourceValue::Present(guid),
            Err(NoObject::MarkerCycle) => SourceValue::Invalid,
            Err(NoObject::MarkerWithoutParent) if contributions.ambiguous_parent => {
                SourceValue::Invalid
            }
            Err(NoObject::NoneAbove | NoObject::NotInCatalog | NoObject::MarkerWithoutParent) => {
                SourceValue::Absent
            }
        };
        let cell = sources.value(state, &[], contributions.complete);
        Ok(sources.add_value_origins(cell, &contributions.origins.iter().collect::<Vec<_>>())?)
    }
}

struct ChainContributions {
    origins: Vec<FieldOrigin>,
    complete: bool,
    ambiguous_parent: bool,
}

impl ChainContributions {
    fn append(
        &mut self,
        object: &LocationObject<'_>,
        sources: &SourceUses,
    ) -> Result<(), CatalogError> {
        self.complete &= object.unique && object.origin.is_some();
        self.origins.extend(object.origin.cloned());
        if object.mining_claim {
            self.append_placement(object, sources)?;
        }
        Ok(())
    }

    fn append_placement(
        &mut self,
        object: &LocationObject<'_>,
        sources: &SourceUses,
    ) -> Result<(), CatalogError> {
        let Some(proof) = object.placement else {
            self.complete = false;
            return Ok(());
        };
        self.ambiguous_parent |= matches!(proof, PlacementEvidence::Ambiguous(_));
        let cell = PlacementBinding::new(sources, Some(proof))?
            .cell(object.above, PlacementField::Parent)?;
        let tested_pose = sources.origin(&proof.parent_selection_inputs()?, true);
        self.origins.push(
            sources.origin_with_origins([cell.origin(), &tested_pose], proof.parent_complete())?,
        );
        Ok(())
    }
}
