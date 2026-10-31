// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Pieces that another worn piece hides: the items at the ports its `HiddenParts` names.

use super::Piece;
use super::jobs::piece_issue;
use super::piece::Source;
use crate::hide_rule::{Facts, hidden_by};
use crate::{Issue, IssueKind};

/// The wearer of every piece of an Armour: the one body.
const WEARER: usize = 0;

fn facts_of(piece: &Piece) -> Facts<'_> {
    let hides = match &piece.source {
        Source::Wearable(wearable) => wearable.hidden_ports(),
        _ => &[],
    };
    Facts::new(
        piece.name(),
        piece.port(),
        WEARER,
        piece
            .mount
            .as_ref()
            .and_then(|mount| mount.holder.as_deref()),
        hides,
    )
}

/// Returns, per piece, whether it is worn, and lists each hidden piece in `issues`.
pub(super) fn worn(pieces: &[Piece], issues: &mut Vec<Issue>) -> Vec<bool> {
    let facts: Vec<Facts<'_>> = pieces.iter().map(facts_of).collect();
    hidden_by(&facts)
        .into_iter()
        .zip(pieces)
        .map(|(hider, piece)| {
            let Some(hider) = hider.and_then(|at| pieces.get(at)) else {
                return true;
            };
            let kind = IssueKind::PartHidden {
                port: piece.port().unwrap_or_default().to_owned(),
                by: hider.name().to_owned(),
            };
            issues.push(piece_issue(piece, kind));
            false
        })
        .collect()
}
