// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_model::Transform;

use crate::build::{Libraries, Library, issue, library_key_of, materials_for};
use crate::build_models::Loaded;
use crate::flatten::Draft;
use crate::placement::bone_transform;
use crate::{AttachedModel, Issue, IssueKind, ProvidedAttachment};

/// Places the bone attachments of the model `loaded` of a part: each at its bone in the bind
/// pose of the part's skeleton, offset by the attachment. A bone the skeleton lacks is an issue,
/// and the model then sits at the part's origin.
pub(crate) fn attached(
    draft: &Draft<'_>,
    loaded: Option<&Loaded>,
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
        .map(|attachment| {
            let key = attachment
                .model
                .material_library
                .as_deref()
                .map(|name| library_key_of(name, &attachment.entry));
            let materials = materials_for(draft, key, libraries, reported, issues);
            let bone = bone_transform(&loaded.model, &attachment.bone);
            if bone.is_none() {
                let helper = attachment.bone.clone();
                issues.push(issue(draft, IssueKind::HelperNotFound { helper }));
            }
            AttachedModel {
                name: attachment.name.clone(),
                bone: attachment.bone.clone(),
                local: bone
                    .unwrap_or(Transform::IDENTITY)
                    .then(&offset_of(attachment)),
                found: bone.is_some(),
                entry: attachment.entry.clone(),
                model: Arc::clone(&attachment.model),
                archive_sources: Arc::clone(&attachment.archive_sources),
                definition_origin: loaded.definition_origin.clone(),
                materials,
            }
        })
        .collect()
}

/// Returns the transform of the attachment's own position and rotation; the reader of the
/// definition lets only finite values and a rotation with length through.
fn offset_of(attachment: &ProvidedAttachment) -> Transform {
    let [w, x, y, z] = attachment.rotation.map(f64::from);
    Transform::from_quaternion([w, x, y, z], attachment.position.map(f64::from))
        .unwrap_or(Transform::IDENTITY)
}
