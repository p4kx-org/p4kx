// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_resources::Resource;

use super::source::{self, Kind};
use super::{CycleEdge, Document};

pub(super) fn find(
    documents: &[Document],
    cancel: &p4kx_extraction::CancelToken,
) -> Result<Vec<CycleEdge>, SocSceneError> {
    let mut outgoing = BTreeMap::<(usize, Kind), Vec<(usize, usize, Kind)>>::new();
    for document in documents {
        for (ordinal, link) in document.links.iter().enumerate() {
            if let (Some(kind), Some(Resource::Present { data, .. })) =
                (link.owner_kind, link.target.value())
            {
                outgoing.entry((document.index, kind)).or_default().push((
                    ordinal,
                    *data,
                    link.target_kind,
                ));
            }
        }
    }
    let mut stack = Vec::new();
    let mut active = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let mut cycles = Vec::new();
    for root in ordered_contexts(documents) {
        if completed.contains(&root) {
            continue;
        }
        stack.push((root, 0));
        active.insert(root);
        while let Some((owner, next)) = stack.last_mut() {
            source::check(cancel)?;
            let Some(&(link, document, kind)) =
                outgoing.get(owner).and_then(|edges| edges.get(*next))
            else {
                active.remove(owner);
                completed.insert(*owner);
                stack.pop();
                continue;
            };
            *next += 1;
            let target = (document, kind);
            if active.contains(&target) {
                cycles.push(CycleEdge {
                    owner: owner.0,
                    kind: owner.1,
                    link,
                });
            } else if !completed.contains(&target) {
                active.insert(target);
                stack.push((target, 0));
            }
        }
    }
    Ok(cycles)
}

fn ordered_contexts(documents: &[Document]) -> impl Iterator<Item = (usize, Kind)> + '_ {
    documents.iter().flat_map(|document| {
        document
            .kinds
            .iter()
            .map(move |&kind| (document.index, kind))
    })
}
