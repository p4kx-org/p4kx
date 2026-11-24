// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_model::{Mesh, Model, Submesh};
use thiserror::Error;

use super::{Armour, BodyVariant, PartOrigin};
use crate::IssueKind;

/// Why the submeshes of a body skin cannot be told apart by zone.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum BodyZoneLayoutError {
    /// The assembly has no body part.
    #[error("the assembly has no body part")]
    MissingBody,
    /// The assembly has more than one body part.
    #[error("the assembly has more than one body part")]
    MultipleBodies,
    /// The selected level does not provide a body mesh.
    #[error("level {level} of the body has no mesh")]
    MissingLevel {
        /// The selected level.
        level: u32,
    },
    /// A level does not number exactly the zones of the table.
    #[error("level {level} of the body has {found} zones, not the {expected} the zone table lists")]
    Nodes {
        /// The level of detail.
        level: u32,
        /// The number of observed zone nodes.
        found: usize,
        /// The number of zones in the variant table.
        expected: usize,
    },
    /// A submesh names indices the mesh does not hold.
    #[error("a submesh of the body names indices beyond its mesh")]
    Range,
    /// A zone is on the wrong side or at the wrong height of the body.
    #[error("the zone {zone} is not where the zone table puts it")]
    Place {
        /// The name whose placement failed.
        zone: &'static str,
    },
}

impl Armour {
    /// Returns the zones omitted from the body's geometry after validating its zone layout.
    ///
    /// An empty slice means the body has an intact, validated layout.
    ///
    /// # Errors
    ///
    /// Returns the reason when the assembly lacks a single verifiable body skin.
    pub fn body_zone_layout(&self, lod: u32) -> Result<&[String], BodyZoneLayoutError> {
        let mut bodies = self
            .parts
            .iter()
            .filter(|part| part.origin == PartOrigin::Body);
        let body = bodies.next().ok_or(BodyZoneLayoutError::MissingBody)?;
        if bodies.next().is_some() {
            return Err(BodyZoneLayoutError::MultipleBodies);
        }
        let selected = body
            .model
            .nearest_lod(lod)
            .filter(|lod| !lod.meshes.is_empty())
            .ok_or(BodyZoneLayoutError::MissingLevel { level: lod })?;
        let names = self.variant.zone_names();
        if let Some(hidden) = self.issues.iter().find_map(|issue| match issue.kind() {
            IssueKind::BodyZonesHidden(zones) => Some(zones.as_slice()),
            _ => None,
        }) {
            for mesh in &selected.meshes {
                check_places(mesh, names)?;
            }
            return Ok(hidden);
        }
        for (index, lod) in body.model.lods.iter().enumerate() {
            for mesh in &lod.meshes {
                check_nodes(mesh, lod.level, names)?;
                if index == 0 || lod.level == selected.level {
                    check_places(mesh, names)?;
                } else {
                    for part in &mesh.submeshes {
                        part_indices(mesh, part)?;
                    }
                }
            }
        }
        Ok(&[])
    }
}

/// A body without the zones that worn clothing covers.
pub(crate) struct Uncovered {
    pub(crate) model: Model,
    pub(crate) hidden: Vec<&'static str>,
}

/// Returns `body` without the triangles of the zones in `covered`, or why its submeshes cannot be
/// told apart by zone: every level must number its submeshes with exactly the node numbers of the
/// variant's zones, and the zones must lie on their side of the body, the feet below the torso.
/// `None` when no zone of the body is covered.
pub(crate) fn uncover(
    body: &Model,
    variant: BodyVariant,
    covered: &BTreeSet<&str>,
) -> Result<Option<Uncovered>, BodyZoneLayoutError> {
    let names = variant.zone_names();
    let hidden: Vec<&'static str> = names
        .iter()
        .copied()
        .filter(|zone| covered.contains(*zone))
        .collect();
    if hidden.is_empty() {
        return Ok(None);
    }
    let mut model = body.clone();
    for (at, lod) in model.lods.iter_mut().enumerate() {
        for mesh in &mut lod.meshes {
            check_nodes(mesh, lod.level, names)?;
            if at == 0 {
                check_places(mesh, names)?;
            }
            keep_only(mesh, |node| {
                names
                    .get(usize::from(node))
                    .is_none_or(|zone| !covered.contains(*zone))
            })?;
        }
    }
    Ok(Some(Uncovered { model, hidden }))
}

fn check_nodes(mesh: &Mesh, level: u32, names: &[&str]) -> Result<(), BodyZoneLayoutError> {
    let present: BTreeSet<u16> = mesh.submeshes.iter().map(|part| part.node_id).collect();
    if present.len() != names.len()
        || !present
            .iter()
            .enumerate()
            .all(|(index, node)| usize::from(*node) == index)
    {
        return Err(BodyZoneLayoutError::Nodes {
            level,
            found: present.len(),
            expected: names.len(),
        });
    }
    Ok(())
}

fn part_indices<'mesh>(
    mesh: &'mesh Mesh,
    part: &Submesh,
) -> Result<&'mesh [u32], BodyZoneLayoutError> {
    part.first_index
        .checked_add(part.index_count)
        .and_then(|end| mesh.indices.get(part.first_index..end))
        .ok_or(BodyZoneLayoutError::Range)
}

/// Rebuilds the triangle list and the submeshes of `mesh` from the submeshes `keep` accepts, so
/// that whoever draws the triangle list sees the cut.
fn keep_only(mesh: &mut Mesh, keep: impl Fn(u16) -> bool) -> Result<(), BodyZoneLayoutError> {
    let mut indices = Vec::with_capacity(mesh.indices.len());
    let mut parts = Vec::with_capacity(mesh.submeshes.len());
    for part in mesh.submeshes.iter().filter(|part| keep(part.node_id)) {
        let slice = part_indices(mesh, part)?;
        let mut moved = *part;
        moved.first_index = indices.len();
        parts.push(moved);
        indices.extend_from_slice(slice);
    }
    if parts.len() != mesh.submeshes.len() {
        mesh.invalidate_vertex_origin_order();
    }
    mesh.indices = indices;
    mesh.submeshes = parts;
    Ok(())
}

/// Checks that the zones lie where their names say: `l_` zones on the negative X side, `r_` on the
/// positive one, and the feet below every torso zone.
fn check_places(mesh: &Mesh, names: &[&'static str]) -> Result<(), BodyZoneLayoutError> {
    let mut lowest_torso = f64::INFINITY;
    let mut highest_foot = f64::NEG_INFINITY;
    for part in &mesh.submeshes {
        let Some(&zone) = names.get(usize::from(part.node_id)) else {
            continue;
        };
        let indices = part_indices(mesh, part)?;
        let (mut x, mut z, mut count) = (0.0_f64, 0.0_f64, 0_u32);
        for &index in indices {
            let at = usize::try_from(index)
                .ok()
                .and_then(|index| mesh.positions.get(index))
                .ok_or(BodyZoneLayoutError::Range)?;
            x += f64::from(at[0]);
            z += f64::from(at[2]);
            count += 1;
        }
        if count == 0 {
            continue;
        }
        let (x, z) = (x / f64::from(count), z / f64::from(count));
        let wrong_side =
            (zone.starts_with("l_") && x >= 0.0) || (zone.starts_with("r_") && x <= 0.0);
        if wrong_side || !x.is_finite() || !z.is_finite() {
            return Err(BodyZoneLayoutError::Place { zone });
        }
        if zone.starts_with("torso") {
            lowest_torso = lowest_torso.min(z);
        }
        if zone.ends_with("_foot_zone") {
            highest_foot = highest_foot.max(z);
        }
    }
    if highest_foot < lowest_torso {
        Ok(())
    } else {
        Err(BodyZoneLayoutError::Place { zone: "foot" })
    }
}
