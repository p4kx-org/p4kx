// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::MaterialStatus;

use crate::bespoke::missing_library;
use crate::build::issue;
use crate::flatten::Draft;
use crate::{Issue, IssueKind, ModelProvider};

/// Gives a part that draws the own skin of a story character's head, see
/// `EntityNode::wear_own_head_skins`, the material library of the head when the library its
/// geometry names is not in the Archive, and notes the replacement. A part that fell back to its
/// geometry keeps its library.
///
/// The library is asked of the provider, not of its files, so that a provider without file access
/// does not report a library that exists.
pub(crate) fn use_head_libraries(
    drafts: &mut [Draft<'_>],
    provider: &dyn ModelProvider,
    issues: &mut Vec<Issue>,
) {
    for draft in drafts.iter_mut().filter(|draft| !draft.hidden) {
        let Some(own) = draft.own_skin() else {
            continue;
        };
        let Some(library) = own.geometry.material() else {
            continue;
        };
        let lacks =
            |name: &str| matches!(provider.library("", name), MaterialStatus::Missing { .. });
        let Some(missing) = missing_library(library, Some(&own.head_library), lacks) else {
            continue;
        };
        draft.library = Some(&own.head_library);
        let reason = missing.reason;
        issues.push(issue(draft, IssueKind::MaterialLibrary { reason }));
    }
}
