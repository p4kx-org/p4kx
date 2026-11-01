// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::Guid;

use crate::geometry_choice::GeometryChoice;
use crate::port::PortDefinition;
use crate::resolver::LoadoutResolver;
use crate::{Attachment, Geometry, GeometryVariant, PortNode, VehicleImplementation};

impl LoadoutResolver<'_> {
    /// Gives the Entity on `node` the sub-geometry its port's flags choose, if there is one; the
    /// geometry it had stays as the fallback. `shared` are the flags the port container of the
    /// Entity that owns the port gives all its ports.
    pub(crate) fn choose_sub_geometry(
        &mut self,
        node: &mut PortNode,
        vehicle: Option<&VehicleImplementation>,
        shared: &[String],
    ) {
        let Attachment::Entity(child) = &mut node.attachment else {
            return;
        };
        let words = flag_words(node.definition.as_deref(), vehicle, &node.name, shared);
        let Some(mut tagged) = self.tagged_geometry(child.guid, &words) else {
            return;
        };
        if let Some(main) = &child.geometry {
            if tagged.material.is_none() {
                tagged.material.clone_from(&main.material);
            }
            if tagged.palette.is_none() {
                tagged.palette.clone_from(&main.palette);
            }
            if tagged.path.eq_ignore_ascii_case(&main.path) {
                child.geometry = Some(tagged);
                child.choice = GeometryChoice::Flag;
                return;
            }
        }
        child.choice = GeometryChoice::Flag;
        child.fallback = child.geometry.replace(tagged);
    }

    /// Returns the sub-geometry of the Entity `guid` whose tags are all among `words`, the flags
    /// of the port it is attached to, such as `left` or `right` for a door; of several, the one
    /// with the most distinct tags, the first on a tie.
    fn tagged_geometry(&mut self, guid: Guid, words: &[&str]) -> Option<Geometry> {
        if words.is_empty() {
            return None;
        }
        let spec = self.spec(guid);
        let spec = spec.as_ref().as_ref().ok()?;
        let mut best: Option<(&GeometryVariant, usize)> = None;
        for variant in &spec.variants {
            let tags = variant.tags();
            let covered = !tags.is_empty()
                && tags
                    .iter()
                    .all(|tag| words.iter().any(|word| word.eq_ignore_ascii_case(tag)));
            if !covered {
                continue;
            }
            let count = distinct(tags);
            if best.is_none_or(|(_, kept)| count > kept) {
                best = Some((variant, count));
            }
        }
        best.map(|(variant, _)| variant.geometry().clone())
    }
}

/// Returns how many tags there are, ignoring ASCII case and repeats.
fn distinct(tags: &[String]) -> usize {
    tags.iter()
        .enumerate()
        .filter(|(at, tag)| {
            !tags
                .iter()
                .take(*at)
                .any(|earlier| earlier.eq_ignore_ascii_case(tag))
        })
        .count()
}

/// Returns the words of the flags of the port `name` without a leading `$`: those of its
/// definition in the Entity Record, else those of the vehicle implementation's item port, then
/// the `shared` flags of the port container.
pub(crate) fn flag_words<'a>(
    definition: Option<&'a PortDefinition>,
    vehicle: Option<&'a VehicleImplementation>,
    name: &str,
    shared: &'a [String],
) -> Vec<&'a str> {
    let defined = definition.map(PortDefinition::flags).unwrap_or_default();
    let flags = if defined.trim().is_empty() {
        vehicle
            .and_then(|vehicle| vehicle.item_port(name))
            .map(crate::VehicleItemPort::flags)
            .unwrap_or_default()
    } else {
        defined
    };
    port_words(flags, shared)
        .map(|word| word.trim_start_matches('$'))
        .filter(|word| !word.is_empty())
        .collect()
}

/// Returns the words of the port's own `flags`, then the `shared` flags of the port container:
/// the flags that choose a sub-geometry.
pub(crate) fn port_words<'a>(
    flags: &'a str,
    shared: &'a [String],
) -> impl Iterator<Item = &'a str> {
    flags
        .split_whitespace()
        .chain(shared.iter().map(String::as_str))
}
