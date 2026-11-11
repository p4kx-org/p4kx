// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use crate::{BODY_COLOR, ChfNameKind, EYE_COLOR, lowered_hash, name_hash, skin_attachment};

/// The longest name a [`NameTable`] keeps; a longer one is not a port, material or parameter name.
pub const MAX_NAME_LEN: usize = 256;

/// The most names of one kind a [`NameTable`] keeps.
pub const MAX_NAMES: usize = 1 << 16;

const PORTS: [&str; 18] = [
    "head_itemport",
    "body_itemport",
    "eyes_itemport",
    "hair_itemport",
    "beard_itemport",
    "eyebrow_itemport",
    "stubble_itemport",
    "hat_itemport",
    "teeth_itemport",
    "eye_accessories_itemport",
    "eyelashes_itemport",
    "eyedetail_itemport",
    "skin_variant",
    "piercings_itemport",
    "piercings_nose_itemport",
    "piercings_eyebrows_itemport",
    "piercings_mouth_itemport",
    "universal_scalp_itemport",
];

const SUB_MATERIALS: [&str; 1] = ["shader_eyeInner"];

const PARAMS: [&str; 3] = ["DyeAmount", "TattooAge", "Makeup2MetalnessR"];

const RIGS: [&str; 2] = [
    "protos_human_male_face_t1_pu",
    "protos_human_female_face_t1_pu",
];

/// The names the hashes of a CHF can be read as, keyed by the hash function of each field (see
/// `docs/chf-hashes.md`).
///
/// A table starts with the names the hash analysis found ([`NameTable::builtin`]); the Archive
/// adds the names of its item ports, material libraries and parameters. A hash that no name
/// matches stays unnamed: the table never guesses.
#[derive(Debug, Clone, Default)]
pub struct NameTable {
    ports: HashMap<u32, String>,
    attachments: HashMap<u32, String>,
    sub_materials: HashMap<u32, String>,
    params: HashMap<u32, String>,
    rigs: HashMap<u32, String>,
}

impl NameTable {
    /// Returns a table with the names found by analysing the shipped Characters.
    #[must_use]
    pub fn builtin() -> Self {
        let mut table = Self::default();
        for port in PORTS {
            table.add_port(port);
        }
        for name in SUB_MATERIALS {
            table.add_sub_material(name);
        }
        for name in PARAMS {
            table.add_param(name);
        }
        table.params.insert(BODY_COLOR, "BodyColor".to_owned());
        table.params.insert(EYE_COLOR, "EyeColor".to_owned());
        for rig in RIGS {
            table.rigs.insert(name_hash(rig), rig.to_owned());
        }
        table
    }

    /// Adds the item port `name`: named by its CRC-32C when lower-cased in an [`crate::ItemPort`],
    /// and by the slot formula in a [`crate::Material`]. Returns whether the table took it.
    pub fn add_port(&mut self, name: &str) -> bool {
        if !fits(name) || self.ports.len() >= MAX_NAMES {
            return false;
        }
        let lower = name.to_ascii_lowercase();
        self.attachments
            .entry(skin_attachment(&lower))
            .or_insert_with(|| lower.clone());
        self.ports.entry(lowered_hash(&lower)).or_insert(lower);
        true
    }

    /// Adds the sub-material `name`, hashed lower-cased. Returns whether the table took it.
    pub fn add_sub_material(&mut self, name: &str) -> bool {
        if !fits(name) || self.sub_materials.len() >= MAX_NAMES {
            return false;
        }
        self.sub_materials
            .entry(lowered_hash(name))
            .or_insert_with(|| name.to_owned());
        true
    }

    /// Adds the parameter `name`, hashed as spelled. Returns whether the table took it.
    pub fn add_param(&mut self, name: &str) -> bool {
        if !fits(name) || self.params.len() >= MAX_NAMES {
            return false;
        }
        self.params
            .entry(name_hash(name))
            .or_insert_with(|| name.to_owned());
        true
    }

    /// Adds the names of `other` this table lacks: existing names win, each kind is merged
    /// apart, and a kind that holds [`MAX_NAMES`] names takes no more.
    pub fn merge(&mut self, other: &Self) {
        let join = |into: &mut HashMap<u32, String>, from: &HashMap<u32, String>| {
            for (hash, name) in from {
                if into.len() >= MAX_NAMES {
                    break;
                }
                into.entry(*hash).or_insert_with(|| name.clone());
            }
        };
        join(&mut self.ports, &other.ports);
        join(&mut self.attachments, &other.attachments);
        join(&mut self.sub_materials, &other.sub_materials);
        join(&mut self.params, &other.params);
        join(&mut self.rigs, &other.rigs);
    }

    /// Returns the exact current lookup winner for this CHF hash family.
    #[must_use]
    pub fn get(&self, kind: ChfNameKind, hash: u32) -> Option<&str> {
        match kind {
            ChfNameKind::Rig => self.rig(hash),
            ChfNameKind::Port => self.port(hash),
            ChfNameKind::Attachment => self.attachment(hash),
            ChfNameKind::SubMaterial => self.sub_material(hash),
            ChfNameKind::Parameter => self.param(hash),
        }
    }

    /// Returns the name of the item port with the hash `hash`.
    #[must_use]
    pub fn port(&self, hash: u32) -> Option<&str> {
        self.ports.get(&hash).map(String::as_str)
    }

    /// Returns the item port that the material attachment hash `hash` names.
    #[must_use]
    pub fn attachment(&self, hash: u32) -> Option<&str> {
        self.attachments.get(&hash).map(String::as_str)
    }

    /// Returns the name of the sub-material with the hash `hash`, lower-cased as it was hashed
    /// unless the table knows another spelling.
    #[must_use]
    pub fn sub_material(&self, hash: u32) -> Option<&str> {
        self.sub_materials.get(&hash).map(String::as_str)
    }

    /// Returns the name of the float or colour parameter with the hash `hash`.
    #[must_use]
    pub fn param(&self, hash: u32) -> Option<&str> {
        self.params.get(&hash).map(String::as_str)
    }

    /// Returns the name of the rig with the DNA gender hash `hash`.
    #[must_use]
    pub fn rig(&self, hash: u32) -> Option<&str> {
        self.rigs.get(&hash).map(String::as_str)
    }
}

const fn fits(name: &str) -> bool {
    !name.is_empty() && name.len() <= MAX_NAME_LEN
}

#[cfg(test)]
mod tests {
    use super::NameTable;

    #[test]
    fn the_builtin_table_names_the_ports_slots_rigs_and_colours_of_the_default_character() {
        let table = NameTable::builtin();

        assert_eq!(table.port(1_191_251_385), Some("head_itemport"));
        assert_eq!(table.attachment(0xa98b_eb34), Some("head_itemport"));
        assert_eq!(table.attachment(0x6c83_6947), Some("hair_itemport"));
        assert_eq!(table.rig(0xdd6c_67f6), Some("protos_human_male_face_t1_pu"));
        assert_eq!(table.param(0xbd53_0797), Some("BodyColor"));
        assert_eq!(table.param(1_257_685_338), Some("DyeAmount"));
        assert_eq!(table.sub_material(2_359_193_884), Some("shader_eyeInner"));
        assert_eq!(table.port(0x6d6d_e693), None);
    }

    #[test]
    fn a_port_is_added_lower_cased_and_over_long_or_empty_names_are_refused() {
        let mut table = NameTable::default();

        assert!(table.add_port("Hair_ItemPort"));
        assert!(!table.add_port(""));
        assert!(!table.add_param(&"x".repeat(257)));

        assert_eq!(table.port(325_065_365), Some("hair_itemport"));
        assert_eq!(table.attachment(0x6c83_6947), Some("hair_itemport"));
    }
}
