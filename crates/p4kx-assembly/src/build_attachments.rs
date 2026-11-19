// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_model::{Transform, attachment_offset};

use crate::attached::MaterialSelection;
use crate::build::{Libraries, Library, SelectedLibrary, issue, library_key_of, materials_for};
use crate::build_models::Loaded;
use crate::flatten::Draft;
use crate::placement;
use crate::{AttachedModel, Geometry, Issue, IssueKind};

/// Places the bone attachments of the model `loaded` of a part: each at its bone in the bind
/// pose of the part's skeleton, offset by the attachment. A bone the skeleton lacks is an issue,
/// and the model then sits at the part's origin.
pub(crate) fn attached(
    draft: &Draft<'_>,
    loaded: Option<&Loaded>,
    primary: &SelectedLibrary,
    libraries: (&Libraries, &Library),
    reported: &mut HashSet<String>,
    issues: &mut Vec<Issue>,
) -> Vec<AttachedModel> {
    let Some(loaded) = loaded else {
        return Vec::new();
    };
    loaded
        .attachments
        .iter()
        .enumerate()
        .map(|(index, attachment)| {
            let (materials, material_selection) = if loaded.rig_only && index == 0 {
                let selection = if primary.from_model {
                    MaterialSelection::Attachment
                } else {
                    let authored = draft
                        .geometry
                        .filter(|_| draft.library.is_none())
                        .and_then(Geometry::material_field);
                    MaterialSelection::Entity(authored)
                };
                (Arc::clone(&primary.materials), selection)
            } else {
                let key = attachment
                    .model
                    .material_library
                    .as_deref()
                    .map(|name| library_key_of(name, &attachment.entry));
                let materials = materials_for(draft, key, libraries, reported, issues);
                (materials, MaterialSelection::Attachment)
            };
            let bone = placement::bone_placement(&loaded.model, &attachment.bone);
            let offset = attachment_offset(attachment.position, attachment.rotation);
            if bone.is_none() {
                let helper = attachment.bone.clone();
                issues.push(issue(draft, IssueKind::HelperNotFound { helper }));
            }
            AttachedModel {
                name: attachment.name.clone(),
                bone: attachment.bone.clone(),
                local: bone
                    .map_or(Transform::IDENTITY, |bone| bone.transform)
                    .then(&offset.unwrap_or(Transform::IDENTITY)),
                found: bone.is_some(),
                bone_selection: bone.map(|bone| bone.selection),
                cdf_sources: attachment.cdf_sources.clone(),
                offset_used: offset.is_some(),
                entry: attachment.entry.clone(),
                model: Arc::clone(&attachment.model),
                archive_sources: Arc::clone(&attachment.archive_sources),
                definition_origin: loaded.definition_origin.clone(),
                materials,
                material_selection,
            }
        })
        .collect()
}
