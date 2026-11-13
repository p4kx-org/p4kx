// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::ModelProvider;
use p4kx_extraction::ArchiveDefaultMaterial;
use p4kx_gltf::DrawnMaterial;
use p4kx_model::MaterialStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::items::Item;
use crate::plan::MeshPlan;

pub(crate) fn select(
    items: &[Item<'_>],
    plan: &MeshPlan,
    level: u32,
    models: &dyn ModelProvider,
) -> Option<Arc<ArchiveDefaultMaterial>> {
    let requires_fallback = (0..plan.len())
        .filter_map(|index| items.get(plan.first(index)?)?.drawn)
        .any(|drawn| {
            drawn
                .model
                .nearest_lod(level)
                .is_some_and(|lod| lod.requires_material_fallback(drawn.materials))
        });
    if requires_fallback {
        models.default_material()
    } else {
        None
    }
}

/// Fallback decision for one raw geometry material ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct FallbackMaterialSlot {
    /// Original geometry material ID.
    pub source_material_id: u16,
    /// Authored library condition that caused the fallback.
    pub cause: FallbackMaterialCause,
}

/// Why an authored material could not fill a geometry slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum FallbackMaterialCause {
    /// The model names no material library.
    UnnamedLibrary,
    /// The named library does not exist.
    MissingLibrary,
    /// The named library could not be read.
    UnreadableLibrary,
    /// The library was skipped by the caller.
    SkippedLibrary,
    /// The source ID selects an explicit empty authored slot.
    EmptySlot,
    /// The source ID has no slot in the loaded library.
    MissingSlot,
    /// The producer could not establish a narrower cause.
    Unknown,
}

pub(crate) fn fallback_slots(
    drawn: &DrawnMaterial,
    authored: &MaterialStatus,
) -> Vec<FallbackMaterialSlot> {
    if drawn.library_index.is_some() {
        return Vec::new();
    }
    drawn
        .source_material_ids
        .iter()
        .map(|&id| FallbackMaterialSlot {
            source_material_id: id,
            cause: cause_of(authored, id),
        })
        .collect()
}

fn cause_of(authored: &MaterialStatus, id: u16) -> FallbackMaterialCause {
    match authored {
        MaterialStatus::Unnamed => FallbackMaterialCause::UnnamedLibrary,
        MaterialStatus::Missing { .. } => FallbackMaterialCause::MissingLibrary,
        MaterialStatus::Unreadable { .. } => FallbackMaterialCause::UnreadableLibrary,
        MaterialStatus::Skipped => FallbackMaterialCause::SkippedLibrary,
        MaterialStatus::Loaded(library) if library.is_empty_slot(id) => {
            FallbackMaterialCause::EmptySlot
        }
        MaterialStatus::Loaded(_) => FallbackMaterialCause::MissingSlot,
        _ => FallbackMaterialCause::Unknown,
    }
}
