// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::sync::Arc;

use p4kx_model::Model;

use crate::armour::{covered, uncovered};
use crate::build::{file_key, issue};
use crate::build_models::Loaded;
use crate::flatten::Draft;
use crate::{BodyVariant, EntityNode, Issue, IssueKind};

const BODY_TYPE: &str = "Char_Body";

/// Returns whether the part is the body of a character: an Entity of the type `Char_Body`.
pub(crate) fn is_body(draft: &Draft<'_>) -> bool {
    draft.node.attach_type().eq_ignore_ascii_case(BODY_TYPE)
}

/// Returns the folder of `path` as a file key, without the file name.
fn folder_key(path: &str) -> String {
    let key = file_key(path);
    key.rsplit_once('\\')
        .map_or_else(String::new, |(folder, _)| folder.to_owned())
}

/// Whether the model of `draft` lies in the folder of the default body skin of `variant`, so a
/// skin of the other body is not cut by this variant's zone table.
fn is_of_variant(draft: &Draft<'_>, variant: BodyVariant) -> bool {
    draft
        .geometry_path()
        .is_some_and(|path| folder_key(path) == folder_key(variant.skin()))
}

/// The cut of a character's body: the zones its worn Entities cover, by the rule of the
/// character package.
pub(crate) struct BodyCut<'t> {
    variant: BodyVariant,
    body: usize,
    covered: BTreeSet<&'t str>,
}

impl<'t> BodyCut<'t> {
    /// Returns the cut for the body the head in `tree` names, from the Entities among `drafts`
    /// that have a model; `None` when the tree has no head or the Entities cover no zone.
    ///
    /// The body is the one `Char_Body` part with a model whose skin lies in the folder of the
    /// variant's body skin. A cut that finds none, several, or one of another variant is reported
    /// in `issues` as [`IssueKind::BodyZonesUnknown`] and not made.
    pub(crate) fn of(
        tree: &EntityNode,
        drafts: &[Draft<'t>],
        chosen: &[Option<&Loaded>],
        issues: &mut Vec<Issue>,
    ) -> Option<Self> {
        let variant = tree.character_body()?.variant();
        let covered = covered(
            drafts
                .iter()
                .zip(chosen)
                .filter(|(_, found)| found.is_some())
                .flat_map(|(draft, _)| draft.node.zones.iter()),
        );
        if covered.is_empty() {
            return None;
        }
        let mut bodies = drafts
            .iter()
            .zip(chosen)
            .enumerate()
            .filter(|(_, (draft, found))| found.is_some() && is_body(draft))
            .map(|(index, (draft, _))| (index, draft));
        let body = match (bodies.next(), bodies.next()) {
            (Some((index, draft)), None) if is_of_variant(draft, variant) => Ok(index),
            (Some((_, draft)), None) => Err((
                draft,
                format!("the {BODY_TYPE} part is not a body skin of the variant"),
            )),
            (Some((_, draft)), Some(_)) => Err((
                draft,
                format!("several parts with a model are a {BODY_TYPE}"),
            )),
            (None, _) => Err((
                drafts.first()?,
                format!("no part with a model is a {BODY_TYPE}"),
            )),
        };
        match body {
            Ok(body) => Some(Self {
                variant,
                body,
                covered,
            }),
            Err((draft, reason)) => {
                issues.push(issue(draft, IssueKind::BodyZonesUnknown(reason)));
                None
            }
        }
    }

    /// Returns the model of the `index`th part, `draft`, as it is worn: the body part without
    /// the covered zones, reported in `issues`; any other model as it is.
    pub(crate) fn model_of(
        &self,
        index: usize,
        draft: &Draft<'_>,
        loaded: &Loaded,
        issues: &mut Vec<Issue>,
    ) -> Arc<Model> {
        let model = Arc::clone(&loaded.model);
        if index != self.body {
            return model;
        }
        let (model, kind) = uncovered(model, self.variant, &self.covered);
        issues.extend(kind.map(|kind| issue(draft, kind)));
        model
    }
}
