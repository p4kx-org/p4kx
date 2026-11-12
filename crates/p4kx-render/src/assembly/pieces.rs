// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_assembly::{Assembly, EntityInterior, INTERIOR_NODE};
use p4kx_extraction::model::{MaterialStatus, Model, TintPalette};

use super::AssemblyPieceSource;
use crate::scene::Rows;

#[cfg(test)]
mod attached_tests;
#[cfg(test)]
mod tests;

/// One thing the scene places: a part of the Loadout, or a part of the Interior with the
/// transform of its container and the hull applied.
pub(super) struct Piece<'a> {
    pub(super) source: AssemblyPieceSource,
    pub(super) name: String,
    pub(super) path: Cow<'a, str>,
    pub(super) parent: Option<usize>,
    pub(super) model: Option<&'a Model>,
    pub(super) world: Rows,
    pub(super) materials: &'a MaterialStatus,
    pub(super) palette: Option<&'a TintPalette>,
    pub(super) interior: bool,
}

/// Lists the pieces of `assembly`: its parts, then the bone attachments of the parts, each below
/// its part, then the parts of the containers of its Interior, each container's root below the
/// part of the Entity that names it.
pub(super) fn pieces_of(assembly: &Assembly) -> Vec<Piece<'_>> {
    let mut pieces: Vec<Piece<'_>> = assembly
        .parts()
        .iter()
        .enumerate()
        .map(|(ordinal, part)| Piece {
            source: AssemblyPieceSource::Part(ordinal),
            name: part.name().to_owned(),
            path: Cow::Borrowed(part.path()),
            parent: part.parent(),
            model: part.model(),
            world: part.world().rows,
            materials: part.materials(),
            palette: part.palette(),
            interior: false,
        })
        .collect();
    for (ordinal, (index, attached)) in assembly.attached_models().enumerate() {
        let Some(part) = assembly.parts().get(index) else {
            continue;
        };
        let path = match part.path() {
            "" => attached.name().to_owned(),
            folder => format!("{folder}/{}", attached.name()),
        };
        pieces.push(Piece {
            source: AssemblyPieceSource::BoneAttachment(ordinal),
            name: format!("{}/{}", part.name(), attached.name()),
            path: Cow::Owned(path),
            parent: Some(index),
            model: Some(attached.model()),
            world: part.world().then(attached.local()).rows,
            materials: attached.materials(),
            palette: part.palette(),
            interior: false,
        });
    }
    if let Some(interior) = assembly.interior() {
        push_containers(&mut pieces, assembly, interior);
    }
    pieces
}

fn push_containers<'a>(
    pieces: &mut Vec<Piece<'a>>,
    assembly: &'a Assembly,
    interior: &'a EntityInterior,
) {
    for (container_ordinal, container) in interior.containers().iter().enumerate() {
        let base = pieces.len();
        let owner = assembly.parts().get(container.part());
        let anchor = owner.map_or_else(
            || *container.placement(),
            |owner| owner.world().then(container.placement()),
        );
        let owner_index = owner.map(|_| container.part());
        let prefix = match owner.map_or("", |owner| owner.path()) {
            "" => INTERIOR_NODE.to_owned(),
            port => format!("{port}/{INTERIOR_NODE}"),
        };
        for (part_ordinal, part) in container.interior().parts().iter().enumerate() {
            pieces.push(Piece {
                source: AssemblyPieceSource::Interior {
                    container: container_ordinal,
                    part: part_ordinal,
                },
                name: format!("{prefix}/{}", part.name()),
                path: Cow::Borrowed(""),
                parent: part
                    .parent()
                    .map_or(owner_index, |parent| Some(base + parent)),
                model: part.model(),
                world: anchor.then(part.world()).rows,
                materials: part.materials(),
                palette: part.palette(),
                interior: true,
            });
        }
    }
}
