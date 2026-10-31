// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{Issue, IssueKind};

/// How many animation names an `animation_not_used` issue lists as examples.
const UNUSED_EXAMPLES: usize = 3;

/// The name of an `animation_not_used` issue that sums up several animations.
const GROUP_NAME: &str = "animations";

/// Replaces the `animation_not_used` issues by one per reason, in the order the reasons first
/// occur and after the other issues. Reasons starting with `limit` count as one reason. An issue
/// for one animation keeps its path and name; one for several is named `animations`.
pub(crate) fn summarise(issues: Vec<Issue>, budget: usize) -> Vec<Issue> {
    let mut rest = Vec::with_capacity(issues.len().min(16));
    let mut groups: Vec<Issue> = Vec::new();
    for issue in issues {
        let Issue { path, name, kind } = issue;
        let IssueKind::AnimationNotUsed { reason, .. } = kind else {
            rest.push(Issue { path, name, kind });
            continue;
        };
        let reason = if reason.starts_with("limit") {
            format!("limit: the animations would exceed the budget of {budget} values")
        } else {
            reason
        };
        let known = groups.iter().position(|group| {
            matches!(&group.kind, IssueKind::AnimationNotUsed { reason: known, .. } if *known == reason)
        });
        match known.and_then(|at| groups.get_mut(at)) {
            Some(group) => {
                if let IssueKind::AnimationNotUsed {
                    count, examples, ..
                } = &mut group.kind
                {
                    *count = count.saturating_add(1);
                    if examples.len() < UNUSED_EXAMPLES {
                        examples.push(name);
                    }
                }
                group.path.clear();
                GROUP_NAME.clone_into(&mut group.name);
            }
            None => groups.push(Issue {
                path,
                name: name.clone(),
                kind: IssueKind::AnimationNotUsed {
                    reason,
                    count: 1,
                    examples: vec![name],
                },
            }),
        }
    }
    rest.extend(groups);
    rest
}
