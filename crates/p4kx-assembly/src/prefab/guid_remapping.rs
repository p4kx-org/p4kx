// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_dataforge::Guid;
use p4kx_model::PrefabGuidRemapping;

use super::resolution_source::OwnedPrefabLocation;
use super::{PrefabInstanceSource, PrefabLinkSource, PrefabLocation, PrefabObjectLocation};

/// Remapping declarations of one nested prefab occurrence.
#[derive(Debug, Clone)]
pub struct PrefabGuidRemappingInstance {
    pub(super) path: Vec<usize>,
    pub(super) owner: OwnedPrefabLocation,
    pub(super) object_index: usize,
    pub(super) entered_target: Option<OwnedPrefabLocation>,
    pub(super) route: Option<Arc<PrefabInstanceSource>>,
    pub(super) lookup: Option<Arc<PrefabLinkSource>>,
}

impl PrefabGuidRemappingInstance {
    /// Returns the zero-based object ordinals from the root to the authoring object.
    #[must_use]
    pub fn instance_path(&self) -> &[usize] {
        &self.path
    }

    /// Returns the physical author of the declarations.
    #[must_use]
    pub fn owner(&self) -> PrefabObjectLocation<'_> {
        PrefabObjectLocation {
            prefab: self.owner.location(),
            object_index: self.object_index,
        }
    }

    /// Returns the target that resolution entered, if any.
    #[must_use]
    pub fn entered_target(&self) -> Option<PrefabLocation<'_>> {
        self.entered_target
            .as_ref()
            .map(OwnedPrefabLocation::location)
    }

    /// Returns the containing route when the resolver retained source evidence.
    #[must_use]
    pub fn containing_route(&self) -> Option<&PrefabInstanceSource> {
        self.route.as_deref()
    }

    /// Returns the result of selecting the nested prefab when source evidence was retained.
    #[must_use]
    pub fn lookup(&self) -> Option<&PrefabLinkSource> {
        self.lookup.as_deref()
    }

    /// Returns the original blocks on this placement.
    #[must_use]
    pub fn blocks(&self) -> &[Vec<PrefabGuidRemapping>] {
        self.owner
            .file
            .library()
            .prefabs
            .get(self.owner.prefab_index)
            .and_then(|prefab| prefab.objects.get(self.object_index))
            .and_then(|object| object.nested.as_ref())
            .map_or(&[], |reference| reference.guid_remapping_blocks.as_slice())
    }

    /// Finds every surviving flattened object matching the original GUID in the entered target.
    pub fn default_objects<'a>(
        &'a self,
        mapping: &PrefabGuidRemapping,
    ) -> impl Iterator<Item = PrefabObjectLocation<'a>> + use<'a> {
        let guid = mapping.default_guid.as_deref().and_then(parse_guid);
        self.entered_target()
            .zip(guid)
            .into_iter()
            .flat_map(move |(target, guid)| {
                target
                    .file
                    .library()
                    .prefabs
                    .get(target.prefab_index)
                    .into_iter()
                    .flat_map(move |prefab| {
                        prefab.objects.iter().enumerate().filter_map(
                            move |(object_index, object)| {
                                (parse_guid(&object.id) == Some(guid)).then_some(
                                    PrefabObjectLocation {
                                        prefab: target,
                                        object_index,
                                    },
                                )
                            },
                        )
                    })
            })
    }
}

fn parse_guid(raw: &str) -> Option<Guid> {
    let raw = raw.trim();
    raw.strip_prefix('{')
        .and_then(|unwrapped| unwrapped.strip_suffix('}'))
        .unwrap_or(raw)
        .parse()
        .ok()
}
