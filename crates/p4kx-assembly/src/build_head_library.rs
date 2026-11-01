// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bespoke::{exists, replaced_library_note};
use crate::build::issue;
use crate::flatten::Draft;
use crate::geometry_choice::GeometryChoice;
use crate::{Issue, IssueKind, ModelProvider};

/// Gives a part that wears the own skin of a story character's head, see
/// `EntityNode::wear_own_head_skins`, the material library of the head when the library its
/// Entity names is not in the Archive, and notes the replacement. A part that fell back to the
/// generic skin keeps its library.
pub(crate) fn use_head_libraries(
    drafts: &mut [Draft<'_>],
    provider: &dyn ModelProvider,
    issues: &mut Vec<Issue>,
) {
    for draft in drafts.iter_mut() {
        let node = draft.node;
        if node.choice != GeometryChoice::Bespoke || draft.hidden {
            continue;
        }
        let (Some(worn), Some(own), Some(head_library)) = (
            draft.geometry,
            node.geometry.as_ref(),
            node.head_library.as_deref(),
        ) else {
            continue;
        };
        let Some(library) = worn.material() else {
            continue;
        };
        if !std::ptr::eq(worn, own) || !library.contains(['/', '\\']) || exists(library, provider) {
            continue;
        }
        let reason = replaced_library_note(library, head_library);
        draft.library = Some(head_library);
        issues.push(issue(draft, IssueKind::MaterialLibrary { reason }));
    }
}
