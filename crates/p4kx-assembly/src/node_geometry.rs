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

    /// Returns the geometry of the [`select_variant`] of the Entity `guid` for the flag `words`
    /// of its port.
    fn tagged_geometry(&mut self, guid: Guid, words: &[&str]) -> Option<Geometry> {
        if words.is_empty() {
            return None;
        }
        let spec = self.spec(guid);
        let spec = spec.as_ref().as_ref().ok()?;
        select_variant(&spec.variants, words).map(|variant| variant.geometry().clone())
    }
}

/// Returns the variant whose tags are all among `words`, ignoring ASCII case; of several, the one
/// with the most distinct tags, the first on a tie. A variant without tags never matches.
pub(crate) fn select_variant<'v>(
    variants: &'v [GeometryVariant],
    words: &[&str],
) -> Option<&'v GeometryVariant> {
    let mut best: Option<(&GeometryVariant, usize)> = None;
    for variant in variants {
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
    best.map(|(variant, _)| variant)
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

#[cfg(test)]
mod tests {
    use super::select_variant;
    use crate::{Geometry, GeometryVariant};

    fn variant(tags: &str, path: &str) -> GeometryVariant {
        GeometryVariant {
            tags: tags.split_whitespace().map(str::to_owned).collect(),
            geometry: Geometry {
                path: path.to_owned(),
                material: None,
                palette: None,
            },
        }
    }

    fn chosen<'v>(variants: &'v [GeometryVariant], words: &[&str]) -> Option<&'v str> {
        select_variant(variants, words).map(|variant| variant.geometry().path())
    }

    #[test]
    fn a_variant_needs_all_its_tags_among_the_words() {
        let variants = [variant("front left", "a"), variant("left", "b")];
        assert_eq!(chosen(&variants, &["left"]), Some("b"));
        assert_eq!(chosen(&variants, &["front"]), None);
    }

    #[test]
    fn the_variant_with_the_most_distinct_tags_wins_and_the_first_on_a_tie() {
        let variants = [
            variant("left", "one"),
            variant("center left", "two"),
            variant("right left", "three"),
        ];
        assert_eq!(chosen(&variants, &["left", "center", "right"]), Some("two"));
    }

    #[test]
    fn case_and_repeated_tags_do_not_count() {
        let variants = [variant("RIGHT", "single"), variant("Left left", "doubled")];
        assert_eq!(chosen(&variants, &["LEFT", "right"]), Some("single"));
    }

    #[test]
    fn a_variant_without_tags_never_matches() {
        let variants = [variant("", "untagged")];
        assert_eq!(chosen(&variants, &["left"]), None);
        assert_eq!(chosen(&variants, &[]), None);
    }

    #[test]
    fn a_paint_tag_matches_like_any_other() {
        let variants = [
            variant("Paint_Base_Red", "red"),
            variant("Paint_Base_Blue", "blue"),
        ];
        assert_eq!(chosen(&variants, &["Paint_Base_Blue"]), Some("blue"));
    }
}
