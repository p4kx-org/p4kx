// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{Guid, SourceUse, SourceUseRole, Value};

use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::commodity_inputs::CommodityInputs;
use crate::crafting_tier::InterpretedTier;
use crate::crafting_types::Cost;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::{CatalogError, ExportAccountingError};

/// Original selector occurrences shared across all resources of one physical Blueprint.
#[derive(Debug)]
pub(crate) struct BlueprintResources {
    record_index: u32,
    leaves: BTreeMap<Guid, Vec<u64>>,
    nodes: BTreeMap<u64, Selector>,
}

#[derive(Debug)]
struct Selector {
    parent: Option<u64>,
    used: SourceUse,
    concrete_type: Option<SourceUse>,
}

impl BlueprintResources {
    pub(crate) fn targets(&self) -> impl Iterator<Item = Guid> + '_ {
        self.leaves.keys().copied()
    }

    pub(crate) fn observe(
        &self,
        wanted: Guid,
        inputs: &mut CommodityInputs<'_>,
    ) -> Result<bool, CatalogError> {
        let Some(leaves) = self.leaves.get(&wanted) else {
            return Ok(false);
        };
        for field in [
            RecordHeaderInput::Field(RecordHeaderField::SourceType),
            RecordHeaderInput::NormalizedPath,
            RecordHeaderInput::Field(RecordHeaderField::RecordName),
        ] {
            inputs.header_input(self.record_index, field, SourceUseRole::Selector)?;
        }
        let mut seen = BTreeSet::new();
        for leaf in leaves {
            let mut next = Some(*leaf);
            while let Some(ordinal) = next {
                if !seen.insert(ordinal) {
                    break;
                }
                let node = self
                    .nodes
                    .get(&ordinal)
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                inputs.extend(
                    std::iter::once(node.used.clone()).chain(node.concrete_type.clone()),
                    true,
                );
                next = node.parent;
            }
        }
        Ok(true)
    }
}

pub(super) fn read(
    tiers: &[InterpretedTier<'_, '_>],
    fields: &OriginalFlowFields<'_, '_, '_>,
) -> Result<BlueprintResources, CatalogError> {
    let mut resources = BlueprintResources {
        record_index: fields.record_index(),
        leaves: BTreeMap::new(),
        nodes: BTreeMap::new(),
    };
    for tier in tiers {
        for (cost, original) in tier.tier.costs().iter().zip(&tier.costs) {
            if let Cost::Resource(cost) = cost
                && let Some(guid) = cost.resource
            {
                let parent = fields.fact(original)?;
                let (_, leaf) = fields
                    .field(Some(parent), "resource")?
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                if crate::record_fields::record_reference(leaf.value()).map(|guid| guid.0)
                    != Some(guid)
                {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
                resources
                    .leaves
                    .entry(guid)
                    .or_default()
                    .push(leaf.ordinal());
                retain_chain(fields, leaf, &mut resources.nodes)?;
            }
        }
    }
    Ok(resources)
}

fn retain_chain(
    fields: &OriginalFlowFields<'_, '_, '_>,
    leaf: &OriginalValueFact<'_, '_, '_>,
    nodes: &mut BTreeMap<u64, Selector>,
) -> Result<(), CatalogError> {
    let mut next = Some(leaf.ordinal());
    while let Some(ordinal) = next {
        fields.check_cancel()?;
        let std::collections::btree_map::Entry::Vacant(node) = nodes.entry(ordinal) else {
            break;
        };
        let index = usize::try_from(ordinal).map_err(|_| ExportAccountingError::CounterOverflow)?;
        let fact = fields
            .facts()
            .get(index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if fact.ordinal() != ordinal {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let concrete_type = matches!(fact.value(), Value::Struct(_))
            .then(|| fact.observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector))
            .transpose()?;
        let parent = fact.parent_ordinal();
        node.insert(Selector {
            parent,
            used: fact.observe(OriginalValuePart::Value, SourceUseRole::Selector)?,
            concrete_type,
        });
        next = parent;
    }
    Ok(())
}
