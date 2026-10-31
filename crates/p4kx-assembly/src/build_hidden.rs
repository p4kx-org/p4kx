// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The parts of a Loadout tree that another worn Entity hides by its `HiddenParts`.

use crate::armour::is_worn_type;
use crate::build::issue;
use crate::build_zones::is_body;
use crate::flatten::{Draft, Flat};
use crate::hide_rule::{Facts, hidden_by};
use crate::placement::is_character_item;
use crate::{IssueKind, PortNode};

/// Returns, per draft, the draft of the body it is worn on: the nearest body above it, else the
/// root. A hider affects only the drafts of its own wearer.
fn wearers(drafts: &[Draft<'_>]) -> Vec<usize> {
    let mut wearers: Vec<usize> = Vec::with_capacity(drafts.len());
    for draft in drafts {
        let wearer = draft
            .parent
            .map_or(0, |parent| wearer_below(drafts, &wearers, parent));
        wearers.push(wearer);
    }
    wearers
}

/// Returns the wearer of an item attached below the draft `parent`: the parent itself if it is a
/// body, else the wearer of the parent.
fn wearer_below(drafts: &[Draft<'_>], wearers: &[usize], parent: usize) -> usize {
    if drafts.get(parent).is_some_and(is_body) {
        parent
    } else {
        wearers.get(parent).copied().unwrap_or(0)
    }
}

/// Returns what decides whether `draft` is hidden: the name of the port it sits at, if it is a
/// character item, and the ports its Entity hides, if it is worn clothing or armour. The root, a
/// landing gear and any other Entity, such as a weapon, hide nothing and are never hidden.
fn facts_of<'t>(draft: &'t Draft<'_>, wearer: usize) -> Facts<'t> {
    let item_type = draft.node.attach_type();
    let own = draft.gear.is_none();
    let port = draft
        .port
        .filter(|_| own && is_character_item(item_type))
        .map(PortNode::name);
    let hides = if own && is_worn_type(item_type) {
        draft.node.hidden_ports()
    } else {
        &[]
    };
    Facts::new(&draft.name, port, wearer, None, hides)
}

/// Marks the parts that another part's Record hides as hidden, by the rules of the Armour
/// (ADR-0279, ADR-0304): a port is matched by name ignoring case, only between the worn
/// Entities and the character items of one wearer, and every part hides by its Record whether it
/// is hidden itself or not. A part below a hidden part is hidden too.
///
/// A hidden part is reported as [`IssueKind::PartHidden`] and has no model, so it covers no zone
/// of the body. An attachment that could not be resolved at a hidden port, or below a hidden
/// part, is reported as hidden instead of unresolved.
pub(crate) fn hide_worn(flat: &mut Flat<'_>) {
    if flat
        .drafts
        .iter()
        .all(|draft| draft.node.hidden_ports().is_empty())
    {
        return;
    }
    let wearers = wearers(&flat.drafts);
    let pending = std::mem::take(&mut flat.unresolved);
    let drafts = flat.drafts.len();
    let by = {
        let mut facts: Vec<Facts<'_>> = flat
            .drafts
            .iter()
            .zip(&wearers)
            .map(|(draft, wearer)| facts_of(draft, *wearer))
            .collect();
        facts.extend(pending.iter().map(|lost| {
            let wearer = wearer_below(&flat.drafts, &wearers, lost.owner);
            Facts::new(lost.port, Some(lost.port), wearer, None, &[])
        }));
        hidden_by(&facts)
    };
    let (of_drafts, of_unresolved) = by.split_at_checked(drafts).unwrap_or_default();
    let hiders = hide_drafts(flat, of_drafts);
    for (lost, own) in pending.iter().zip(of_unresolved) {
        let inherited = hiders.get(lost.owner).copied().flatten();
        let hider = own
            .or(inherited)
            .and_then(|hider| flat.drafts.get(hider))
            .map(|hider| hider.name.clone());
        if let (Some(by), Some(found)) = (hider, flat.issues.get_mut(lost.issue)) {
            found.kind = IssueKind::PartHidden {
                port: lost.port.to_owned(),
                by,
            };
        }
    }
}

/// Hides the drafts `by` names a hider for, and the drafts below them, and reports each part that
/// sits at a port. Returns, per draft, the draft that hides it or the part above it.
fn hide_drafts(flat: &mut Flat<'_>, by: &[Option<usize>]) -> Vec<Option<usize>> {
    let mut hiders: Vec<Option<usize>> = Vec::with_capacity(by.len());
    for (index, own) in by.iter().enumerate() {
        let inherited = flat
            .drafts
            .get(index)
            .and_then(|draft| draft.parent)
            .and_then(|parent| hiders.get(parent).copied().flatten());
        let hider = own.or(inherited);
        hiders.push(hider);
        let Some(hider) = hider else { continue };
        let Some(draft) = flat.drafts.get_mut(index).filter(|draft| !draft.hidden) else {
            continue;
        };
        draft.hidden = true;
        let (Some(port), Some(by)) = (
            draft.port.map(PortNode::name),
            flat.drafts.get(hider).map(|draft| draft.name.clone()),
        ) else {
            continue;
        };
        let kind = IssueKind::PartHidden {
            port: port.to_owned(),
            by,
        };
        if let Some(draft) = flat.drafts.get(index) {
            flat.issues.push(issue(draft, kind));
        }
    }
    hiders
}
