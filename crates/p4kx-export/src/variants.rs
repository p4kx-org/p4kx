// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::hash::Hash;
use std::mem;

use crate::collected::Collected;
use crate::digest::Digest;
use crate::guid_text::CatalogGuid;
use crate::item_variant::ItemKey;
use crate::{ExportAccountingError, Playability, VariantCounts, VariantSummary};

/// What the ships of one variant group share (`docs/catalog.md`): the vehicle
/// definition path, compared without regard to case and separator, and the digest of the vehicle
/// implementation after the Modification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ShipKey {
    definition: Box<str>,
    implementation: Digest,
}

impl ShipKey {
    /// Returns the key of a ship with the vehicle definition `definition` and the implementation
    /// digest `implementation`.
    pub(crate) fn new(definition: &str, implementation: Digest) -> Self {
        Self {
            definition: normalised(definition),
            implementation,
        }
    }
}

fn normalised(path: &str) -> Box<str> {
    path.trim_start_matches(['/', '\\'])
        .chars()
        .map(|c| {
            if c == '\\' {
                '/'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect::<String>()
        .into_boxed_str()
}

impl Collected<'_> {
    /// Moves physically bound ship variant keys out of the vehicle facts.
    ///
    /// # Errors
    /// Returns an accounting error if a vehicle has no physical Record binding.
    pub(crate) fn take_ship_variant_keys(
        &mut self,
    ) -> Result<HashMap<u32, ShipKey>, ExportAccountingError> {
        let mut keys = HashMap::new();
        for vehicle in mem::take(&mut self.vehicles) {
            let index = vehicle
                .source_record_index
                .ok_or(ExportAccountingError::MissingFieldInspection)?;
            if let Some(key) = vehicle.variant_key {
                keys.insert(index, key);
            }
        }
        Ok(keys)
    }

    /// Sets `variant_of` of the ships and the items that belong to a variant group and returns the
    /// counts.
    pub(crate) fn group_variants(&mut self, ship_keys: HashMap<u32, ShipKey>) -> VariantSummary {
        let (bases, counts) = {
            let candidates: Vec<Candidate<'_, &ShipKey>> = self
                .ships
                .iter()
                .map(|ship| Candidate {
                    guid: ship.identity.guid,
                    class_name: ship.identity.class_name,
                    playability: ship.playability.value().copied(),
                    key: ship_keys.get(&ship.source_record_index),
                })
                .collect();
            group(&candidates)
        };
        drop(ship_keys);
        for (ship, base) in self.ships.iter_mut().zip(bases) {
            ship.variant_of = base;
        }
        let candidates: Vec<Candidate<'_, ItemKey<'_>>> = self
            .items
            .iter()
            .map(|item| Candidate {
                guid: item.identity.guid,
                class_name: item.identity.class_name,
                playability: item.source.playability().value().copied(),
                key: item.variant_key,
            })
            .collect();
        let (bases, item_counts) = group(&candidates);
        for (item, base) in self.items.iter_mut().zip(bases) {
            item.variant_of = base;
        }
        VariantSummary {
            ships: counts,
            items: item_counts,
        }
    }
}

/// An entity that may belong to a variant group: its identity, the playability that ranks it as a
/// base and the structural key every member of its group shares, `None` when it cannot be
/// grouped.
pub(crate) struct Candidate<'a, K> {
    pub(crate) guid: CatalogGuid,
    pub(crate) class_name: &'a str,
    pub(crate) playability: Option<Playability>,
    pub(crate) key: Option<K>,
}

/// Returns, for every candidate in order, the GUID of the base of its variant group, `None` for a
/// base and for a candidate without a key or alone in its group, with the counts of the groups.
///
/// The base of a group is its member with playability `player` if there is one, then the member
/// with the smallest class name, then the smallest GUID text.
pub(crate) fn group<K: Hash + Eq>(
    candidates: &[Candidate<'_, K>],
) -> (Vec<Option<CatalogGuid>>, VariantCounts) {
    let mut groups: HashMap<&K, Vec<usize>> = HashMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        if let Some(key) = &candidate.key {
            groups.entry(key).or_default().push(index);
        }
    }
    let mut bases = vec![None; candidates.len()];
    let mut counts = VariantCounts::default();
    for members in groups.values().filter(|members| members.len() > 1) {
        let base = members
            .iter()
            .filter_map(|index| candidates.get(*index))
            .min_by_key(|candidate| {
                (
                    candidate.playability != Some(Playability::Player),
                    candidate.class_name,
                    candidate.guid.text(),
                )
            });
        let Some(base) = base else { continue };
        let mut variants = 0_usize;
        for index in members {
            let Some(candidate) = candidates.get(*index) else {
                continue;
            };
            if candidate.guid != base.guid
                && let Some(slot) = bases.get_mut(*index)
            {
                *slot = Some(base.guid);
                variants = variants.saturating_add(1);
            }
        }
        if variants > 0 {
            counts.groups = counts.groups.saturating_add(1);
            counts.variants = counts.variants.saturating_add(variants);
        }
    }
    (bases, counts)
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::Guid;

    use super::*;

    fn candidate(class_name: &str, playability: Option<Playability>) -> Candidate<'_, u8> {
        Candidate {
            guid: CatalogGuid(Guid::default()),
            class_name,
            playability,
            key: Some(1),
        }
    }

    #[test]
    fn a_player_member_is_the_base_before_a_smaller_class_name() {
        let ai = candidate("A_Ai", Some(Playability::Ai));
        let mut player = candidate("B_Player", Some(Playability::Player));
        player.guid = CatalogGuid(Guid::from_stored([1; 16]));
        let player_guid = player.guid;
        let (bases, counts) = group(&[ai, player]);
        assert_eq!(bases, vec![Some(player_guid), None]);
        assert_eq!(
            counts,
            VariantCounts {
                groups: 1,
                variants: 1
            }
        );
    }

    #[test]
    fn equal_class_names_and_playability_fall_back_to_the_smaller_guid() {
        let mut high = candidate("Same", None);
        high.guid = CatalogGuid(Guid::from_stored([9; 16]));
        let mut low = candidate("Same", None);
        low.guid = CatalogGuid(Guid::from_stored([1; 16]));
        let (low_text, high_text) = (low.guid.text(), high.guid.text());
        let low_guid = low.guid;
        let (bases, _) = group(&[high, low]);
        let expected = if low_text < high_text {
            low_guid
        } else {
            CatalogGuid(Guid::from_stored([9; 16]))
        };
        assert_eq!(bases.iter().flatten().next(), Some(&expected));
    }

    #[test]
    fn a_duplicate_of_the_base_guid_is_no_variant_and_makes_no_group() {
        let (bases, counts) = group(&[candidate("A", None), candidate("A", None)]);
        assert_eq!(bases, vec![None, None]);
        assert_eq!(counts, VariantCounts::default());
    }

    #[test]
    fn a_duplicate_of_the_base_guid_beside_a_real_variant_counts_only_the_variant() {
        let mut other = candidate("B", None);
        other.guid = CatalogGuid(Guid::from_stored([1; 16]));
        let base_guid = CatalogGuid(Guid::default());
        let (bases, counts) = group(&[candidate("A", None), candidate("A", None), other]);
        assert_eq!(bases, vec![None, None, Some(base_guid)]);
        assert_eq!(
            counts,
            VariantCounts {
                groups: 1,
                variants: 1
            }
        );
    }

    #[test]
    fn ship_keys_ignore_case_separator_and_a_leading_separator() {
        let digest = Digest::from_bytes([3; 16]);
        let key = ShipKey::new("Scripts/Entities/Vehicles/Alpha.xml", digest);
        assert_eq!(
            key,
            ShipKey::new("\\scripts\\entities\\VEHICLES\\alpha.XML", digest)
        );
        assert_ne!(
            key,
            ShipKey::new("scripts/entities/vehicles/beta.xml", digest)
        );
        assert_ne!(
            key,
            ShipKey::new(
                "scripts/entities/vehicles/alpha.xml",
                Digest::from_bytes([4; 16])
            )
        );
    }

    #[test]
    fn a_candidate_without_a_key_stays_ungrouped() {
        let keyless = Candidate {
            key: None,
            ..candidate("A", None)
        };
        let (bases, counts) = group(&[keyless, candidate("B", None)]);
        assert_eq!(bases, vec![None, None]);
        assert_eq!(counts, VariantCounts::default());
    }
}
