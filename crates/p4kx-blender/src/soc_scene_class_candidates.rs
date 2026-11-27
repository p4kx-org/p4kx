// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet, HashSet};

use p4kx_assembly::{
    SocContainerCapture, SocLightClassSelector, SocMemberSelection, SocSourceGuid,
};
use p4kx_extraction::{CancelToken, Guid};
use p4kx_model::{SocEntity, SocEntityComponentKind, SocEntityFile};

use crate::soc_scene::SocSceneError;
use crate::soc_scene_classes::check;
use crate::soc_scene_light_classes::Selector;

#[derive(Default)]
pub(crate) struct Candidates {
    pub(crate) entities: BTreeMap<Guid, bool>,
    pub(crate) lights: BTreeSet<Selector>,
}

pub(crate) fn inline_candidate(entity: SocEntity<'_>) -> bool {
    entity.authored_components().any(|component| {
        matches!(
            component.kind(),
            SocEntityComponentKind::Door | SocEntityComponentKind::DefaultLoadout
        ) || component.is_proximity_sensor()
    })
}

pub(crate) fn candidates(
    capture: &SocContainerCapture,
    cancel: &CancelToken,
) -> Result<Candidates, SocSceneError> {
    let mut wanted = Candidates::default();
    let mut seen = HashSet::new();
    for stream in capture
        .occurrences()
        .iter()
        .flat_map(|occurrence| &occurrence.streams)
    {
        check(cancel)?;
        let Some(member) = stream
            .member
            .as_ref()
            .and_then(SocMemberSelection::selected)
        else {
            continue;
        };
        if !seen.insert(member.origin().binding()) {
            continue;
        }
        let Some(file) = member
            .payload()
            .ok()
            .and_then(|bytes| SocEntityFile::parse(bytes).ok())
        else {
            continue;
        };
        for chunk in file.entity_chunks().flatten() {
            check(cancel)?;
            for entity in chunk.entities() {
                if let SocSourceGuid::Present(guid) =
                    SocSourceGuid::read(entity.class_guid_attributes())
                {
                    *wanted.entities.entry(guid).or_insert(false) |= inline_candidate(entity);
                }
                if entity.authored_lights().next().is_some() {
                    wanted
                        .lights
                        .extend(Selector::read(SocLightClassSelector::read(entity)));
                }
            }
        }
    }
    Ok(wanted)
}
