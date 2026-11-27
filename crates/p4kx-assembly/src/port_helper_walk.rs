// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::port_sub_helper::{AuthoredHelperTags, HelperChildren, PortSubHelper};
use crate::{PortDefinition, PortTransform};

use super::{
    PortHelperChoice, PortHelperObservation, PortHelperPath, PortHelperProblem,
    PortHelperSelection, PortHelperTagMatch,
};

#[derive(Default)]
struct Branch {
    visit_children: bool,
    requires_helper: bool,
    has_helper: bool,
}

struct Failure<'definition> {
    path: PortHelperPath<'definition>,
    problem: PortHelperProblem,
}

type SelectionResult<'definition, T> = Result<T, Failure<'definition>>;

pub(super) fn select(
    definition: &PortDefinition,
    mut evaluate: impl FnMut(&str) -> PortHelperTagMatch,
) -> PortHelperSelection<'_> {
    let helpers = definition.sub_helpers();
    let root = PortHelperPath {
        helpers,
        index: None,
    };
    let mut observations = Vec::new();
    let choice = choose(definition, root, &mut evaluate, &mut observations)
        .unwrap_or_else(|Failure { path, problem }| PortHelperChoice::Unresolved { path, problem });
    PortHelperSelection {
        choice,
        observations,
    }
}

fn choose<'definition>(
    definition: &'definition PortDefinition,
    root: PortHelperPath<'definition>,
    evaluate: &mut impl FnMut(&str) -> PortHelperTagMatch,
    observations: &mut Vec<PortHelperObservation<'definition>>,
) -> SelectionResult<'definition, PortHelperChoice<'definition>> {
    observe_root(definition, root, observations)?;
    let mut branches: Vec<Branch> = Vec::with_capacity(root.helpers.len());
    let mut matches = Vec::new();
    for (index, node) in root.helpers.iter().enumerate() {
        let path = root.at(index);
        if node.parent().is_some_and(|parent| {
            !branches
                .get(parent)
                .is_some_and(|branch| branch.visit_children)
        }) {
            branches.push(Branch::default());
            continue;
        }
        let evaluation = observe(path, &node.tags, evaluate, observations)?;
        if evaluation == Some(PortHelperTagMatch::NotMatched) {
            branches.push(Branch::default());
            continue;
        }
        path.check_children(node.children)?;
        let condition_matches = evaluation == Some(PortHelperTagMatch::Matched);
        let transform = if condition_matches {
            usable_transform(node, path)?
        } else {
            None
        };
        branches.push(Branch {
            visit_children: true,
            requires_helper: condition_matches,
            has_helper: transform.is_some(),
        });
        if let Some(transform) = transform {
            matches.push((path, transform));
        }
    }
    check_matching_branches(root, &mut branches)?;
    Ok(match matches.as_slice() {
        [] => PortHelperChoice::Root(definition.transform()),
        [(path, transform)] => PortHelperChoice::Selected {
            transform,
            path: *path,
        },
        _ => PortHelperChoice::Ambiguous(matches.into_iter().map(|(path, _)| path).collect()),
    })
}

fn observe_root<'definition>(
    definition: &'definition PortDefinition,
    root: PortHelperPath<'definition>,
    observations: &mut Vec<PortHelperObservation<'definition>>,
) -> SelectionResult<'definition, ()> {
    let tree = &definition.helper_tree;
    let text = tree.root_tags.text();
    let problem = match &tree.root_tags {
        _ if tree.root_unreadable => Some(PortHelperProblem::UnreadableTree),
        AuthoredHelperTags::Unreadable => Some(PortHelperProblem::UnreadableSelector),
        AuthoredHelperTags::Text(text) if !text.trim().is_empty() => {
            Some(PortHelperProblem::TaggedRoot)
        }
        AuthoredHelperTags::Absent | AuthoredHelperTags::Text(_) => None,
    };
    observations.push(PortHelperObservation {
        path: root,
        text,
        evaluation: None,
        inspected_children: problem.is_none(),
    });
    if let Some(problem) = problem {
        return Err(root.failure(problem));
    }
    root.check_children(tree.root_children)
}

fn observe<'definition>(
    path: PortHelperPath<'definition>,
    tags: &'definition AuthoredHelperTags,
    evaluate: &mut impl FnMut(&str) -> PortHelperTagMatch,
    observations: &mut Vec<PortHelperObservation<'definition>>,
) -> SelectionResult<'definition, Option<PortHelperTagMatch>> {
    let text = tags.text();
    let evaluation = text.filter(|text| !text.trim().is_empty()).map(evaluate);
    let inspected_children = !matches!(tags, AuthoredHelperTags::Unreadable)
        && matches!(evaluation, None | Some(PortHelperTagMatch::Matched));
    observations.push(PortHelperObservation {
        path,
        text,
        evaluation,
        inspected_children,
    });
    if matches!(tags, AuthoredHelperTags::Unreadable) {
        return Err(path.failure(PortHelperProblem::UnreadableSelector));
    }
    if evaluation == Some(PortHelperTagMatch::Unsupported) {
        return Err(path.failure(PortHelperProblem::UnsupportedSelector));
    }
    Ok(evaluation)
}

fn usable_transform<'definition>(
    node: &'definition PortSubHelper,
    path: PortHelperPath<'definition>,
) -> SelectionResult<'definition, Option<&'definition PortTransform>> {
    if node.helper_unreadable {
        return Err(path.failure(PortHelperProblem::UnusableHelper));
    }
    let transform = node.transform();
    if transform.is_some_and(|transform| {
        transform.helper().trim().is_empty()
            || !transform
                .position()
                .into_iter()
                .chain(transform.rotation())
                .all(f32::is_finite)
    }) {
        return Err(path.failure(PortHelperProblem::UnusableHelper));
    }
    Ok(transform)
}

fn check_matching_branches<'definition>(
    root: PortHelperPath<'definition>,
    branches: &mut [Branch],
) -> SelectionResult<'definition, ()> {
    for (index, node) in root.helpers.iter().enumerate().rev() {
        let Some(branch) = branches.get(index) else {
            return Err(root.at(index).failure(PortHelperProblem::UnusableHelper));
        };
        if branch.requires_helper && !branch.has_helper {
            return Err(root.at(index).failure(PortHelperProblem::UnusableHelper));
        }
        if branch.has_helper
            && let Some(parent) = node.parent().and_then(|i| branches.get_mut(i))
        {
            parent.has_helper = true;
        }
    }
    Ok(())
}

impl<'definition> PortHelperPath<'definition> {
    const fn at(self, index: usize) -> Self {
        Self {
            index: Some(index),
            ..self
        }
    }

    const fn failure(self, problem: PortHelperProblem) -> Failure<'definition> {
        Failure {
            path: self,
            problem,
        }
    }

    const fn check_children(self, children: HelperChildren) -> SelectionResult<'definition, ()> {
        let problem = match children {
            HelperChildren::Read => return Ok(()),
            HelperChildren::Unreadable => PortHelperProblem::UnreadableChildren,
            HelperChildren::UnreadableSlot(ordinal) => PortHelperProblem::UnreadableChild(ordinal),
        };
        Err(self.failure(problem))
    }
}
