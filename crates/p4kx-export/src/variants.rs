// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::hash::Hash;

use crate::digest::Digest;
use crate::guid_text::{CatalogGuid, GuidText};
use crate::{Playability, VariantCounts};

/// What the ships of one variant group share (`docs/catalog.md`): the vehicle
/// definition path, compared without regard to case and separator, and the digest of the vehicle
/// implementation after the Modification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ShipKey {
    definition: ShipDefinitionKey,
    implementation: Digest,
}

impl ShipKey {
    /// Builds the grouping key from a nonempty original definition and an effective digest.
    pub(crate) fn from_read(
        definition: Option<&str>,
        implementation: Option<Digest>,
    ) -> Option<Self> {
        Some(Self::new(
            definition.filter(|path| !path.is_empty())?,
            implementation?,
        ))
    }

    /// Returns the key of a ship with the vehicle definition `definition` and the implementation
    /// digest `implementation`.
    pub(crate) fn new(definition: &str, implementation: Digest) -> Self {
        Self {
            definition: ShipDefinitionKey::new(definition),
            implementation,
        }
    }
}

/// The original definition path component of a Ship variant key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ShipDefinitionKey(Box<str>);

impl ShipDefinitionKey {
    pub(crate) fn new(path: &str) -> Self {
        Self(
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
                .into_boxed_str(),
        )
    }
}

/// The physical Record of a variant group member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VariantMember {
    pub(crate) guid: CatalogGuid,
    pub(crate) record_index: u32,
}

/// A physical variant group candidate with its rank and optional group key.
pub(crate) struct Candidate<'a, K> {
    pub(crate) member: VariantMember,
    pub(crate) class_name: &'a str,
    pub(crate) playability: Option<Playability>,
    pub(crate) key: Option<K>,
}

#[derive(Clone, Copy)]
pub(crate) struct VariantRank<'source> {
    pub(crate) member: VariantMember,
    pub(crate) class_name: &'source str,
    pub(crate) playability: Option<Playability>,
}

impl<'source> VariantRank<'source> {
    fn order(self) -> (bool, &'source str, GuidText, u32) {
        (
            self.playability != Some(Playability::Player),
            self.class_name,
            self.member.guid.text(),
            self.member.record_index,
        )
    }
}

struct VariantGroup<'source> {
    base: VariantRank<'source>,
    has_variant: bool,
}

/// Retains the best physical candidate and distinct-GUID status per grouping key.
pub(crate) struct VariantBases<'source, K> {
    groups: HashMap<K, VariantGroup<'source>>,
}

impl<'source, K: Eq + Hash> VariantBases<'source, K> {
    pub(crate) fn new() -> Self {
        Self {
            groups: HashMap::new(),
        }
    }

    pub(crate) fn observe(&mut self, key: K, candidate: VariantRank<'source>) {
        self.groups
            .entry(key)
            .and_modify(|group| {
                group.has_variant |= group.base.member.guid != candidate.member.guid;
                if candidate.order() < group.base.order() {
                    group.base = candidate;
                }
            })
            .or_insert(VariantGroup {
                base: candidate,
                has_variant: false,
            });
    }

    pub(crate) fn base_for(&self, key: &K, self_guid: CatalogGuid) -> Option<VariantMember> {
        let base = self.groups.get(key)?.base.member;
        (base.guid != self_guid).then_some(base)
    }

    fn group_count(&self) -> usize {
        self.groups
            .values()
            .filter(|group| group.has_variant)
            .count()
    }
}

/// Returns the physical base of each variant in candidate order, with the group counts.
pub(crate) fn group<K: Hash + Eq>(
    candidates: &[Candidate<'_, K>],
) -> (Vec<Option<VariantMember>>, VariantCounts) {
    let mut choices = VariantBases::new();
    for candidate in candidates {
        if let Some(key) = &candidate.key {
            choices.observe(
                key,
                VariantRank {
                    member: candidate.member,
                    class_name: candidate.class_name,
                    playability: candidate.playability,
                },
            );
        }
    }
    let bases: Vec<_> = candidates
        .iter()
        .map(|candidate| {
            let key = candidate.key.as_ref()?;
            choices.base_for(&key, candidate.member.guid)
        })
        .collect();
    let counts = VariantCounts {
        groups: choices.group_count(),
        variants: bases.iter().flatten().count(),
    };
    (bases, counts)
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::Guid;

    use super::{Candidate, ShipKey, VariantMember};
    use crate::digest::Digest;
    use crate::guid_text::CatalogGuid;
    use crate::{Playability, VariantCounts};

    fn candidate(
        record_index: u32,
        class_name: &str,
        playability: Option<Playability>,
    ) -> Candidate<'_, u8> {
        Candidate {
            member: VariantMember {
                guid: CatalogGuid(Guid::default()),
                record_index,
            },
            class_name,
            playability,
            key: Some(1),
        }
    }

    #[test]
    fn a_player_member_is_the_base_before_a_smaller_class_name() {
        let ai = candidate(0, "A_Ai", Some(Playability::Ai));
        let mut player = candidate(1, "B_Player", Some(Playability::Player));
        player.member.guid = CatalogGuid(Guid::from_stored([1; 16]));
        let player_member = player.member;
        let (bases, counts) = super::group(&[ai, player]);
        assert_eq!(
            bases,
            [Some(player_member), None],
            "player rank chooses the base"
        );
        assert_eq!(
            counts,
            VariantCounts {
                groups: 1,
                variants: 1,
            },
            "one member has a variant link",
        );
    }

    #[test]
    fn equal_class_names_and_playability_fall_back_to_the_smaller_guid() {
        let mut high = candidate(0, "Same", None);
        high.member.guid = CatalogGuid(Guid::from_stored([9; 16]));
        let mut low = candidate(1, "Same", None);
        low.member.guid = CatalogGuid(Guid::from_stored([1; 16]));
        let expected = low.member;
        let (bases, _) = super::group(&[high, low]);
        assert_eq!(
            bases.iter().flatten().next(),
            Some(&expected),
            "GUID text ranks before the physical index",
        );
    }

    #[test]
    fn a_duplicate_of_the_base_guid_is_no_variant_and_makes_no_group() {
        let (bases, counts) = super::group(&[candidate(0, "A", None), candidate(1, "A", None)]);
        assert_eq!(bases, [None, None], "aliases have no self-links");
        assert_eq!(
            counts,
            VariantCounts::default(),
            "a group needs a variant link"
        );
    }

    #[test]
    fn tied_aliases_choose_the_first_physical_base_in_either_input_order() {
        for (first_index, second_index) in [(5, 17), (17, 5)] {
            let first = candidate(first_index, "A", None);
            let second = candidate(second_index, "A", None);
            let expected = VariantMember {
                guid: first.member.guid,
                record_index: 5,
            };
            let mut other = candidate(8, "B", None);
            other.member.guid = CatalogGuid(Guid::from_stored([1; 16]));
            let (bases, counts) = super::group(&[first, second, other]);
            assert_eq!(
                bases,
                [None, None, Some(expected)],
                "the lower physical index wins a rank tie without linking aliases",
            );
            assert_eq!(
                counts,
                VariantCounts {
                    groups: 1,
                    variants: 1,
                },
                "duplicate aliases do not inflate the variant count",
            );
        }
    }

    #[test]
    fn ship_keys_ignore_case_separator_and_a_leading_separator() {
        let digest = Digest::from_bytes([3; 16]);
        let key = ShipKey::new("Scripts/Entities/Vehicles/Alpha.xml", digest);
        assert_eq!(
            key,
            ShipKey::new("\\scripts\\entities\\VEHICLES\\alpha.XML", digest),
            "normalizes vehicle definition paths",
        );
        assert_ne!(
            key,
            ShipKey::new("scripts/entities/vehicles/beta.xml", digest),
            "different vehicle definitions are distinct keys",
        );
        assert_ne!(
            key,
            ShipKey::new(
                "scripts/entities/vehicles/alpha.xml",
                Digest::from_bytes([4; 16])
            ),
            "different implementations are distinct keys",
        );
    }

    #[test]
    fn a_candidate_without_a_key_stays_ungrouped() {
        let keyless = Candidate {
            key: None,
            ..candidate(0, "A", None)
        };
        let (bases, counts) = super::group(&[keyless, candidate(1, "B", None)]);
        assert_eq!(
            bases,
            [None, None],
            "a missing key cannot join another member"
        );
        assert_eq!(
            counts,
            VariantCounts::default(),
            "a singleton is not a group"
        );
    }
}
