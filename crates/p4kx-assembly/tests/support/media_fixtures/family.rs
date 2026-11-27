// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::EntityKind;

/// The public source families represented by the media canaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    Gladius,
    Avenger,
    Constellation,
    Idris,
    Cyclone,
    SizeThreeWeapon,
    LuxuryShip,
    Hornet,
    Freelancer,
    Cutlass,
    Polaris,
    RacerM50,
    Racer350,
    Gladiator,
    Cutter,
    Archimedes,
}

pub(super) struct Spec {
    pub(super) preferred: &'static [&'static str],
    pub(super) prefixes: &'static [&'static str],
    pub(super) kind: EntityKind,
}

impl Family {
    pub(crate) const ALL: [Self; 16] = [
        Self::Gladius,
        Self::Avenger,
        Self::Constellation,
        Self::Idris,
        Self::Cyclone,
        Self::SizeThreeWeapon,
        Self::LuxuryShip,
        Self::Hornet,
        Self::Freelancer,
        Self::Cutlass,
        Self::Polaris,
        Self::RacerM50,
        Self::Racer350,
        Self::Gladiator,
        Self::Cutter,
        Self::Archimedes,
    ];
    pub(super) const fn spec(self) -> Spec {
        match self {
            Self::Gladius => ship(&["AEGS_Gladius"], &["AEGS_Gladius"]),
            Self::Avenger => ship(&["AEGS_Avenger_Titan"], &["AEGS_Avenger"]),
            Self::Constellation => ship(&["RSI_Constellation_Andromeda"], &["RSI_Constellation"]),
            Self::Idris => ship(&["AEGS_Idris_P"], &["AEGS_Idris"]),
            Self::LuxuryShip => ship(&["ORIG_890Jump"], &["ORIG_890Jump"]),
            Self::Hornet => ship(&["ANVL_Hornet_F7C"], &["ANVL_Hornet"]),
            Self::Freelancer => ship(&["MISC_Freelancer"], &["MISC_Freelancer"]),
            Self::Cutlass => ship(&["DRAK_Cutlass_Black"], &["DRAK_Cutlass"]),
            Self::Polaris => ship(&["RSI_Polaris"], &["RSI_Polaris"]),
            Self::RacerM50 => ship(&["ORIG_M50"], &["ORIG_M50"]),
            Self::Racer350 => ship(&["ORIG_350r"], &["ORIG_350r"]),
            Self::Gladiator => ship(&["ANVL_Gladiator"], &["ANVL_Gladiator"]),
            Self::Cutter => ship(&["DRAK_Cutter"], &["DRAK_Cutter"]),
            Self::Archimedes => ship(&["KRIG_P72_Archimedes"], &["KRIG_P72"]),
            Self::Cyclone => Spec {
                preferred: &["TMBL_Cyclone"],
                prefixes: &["TMBL_Cyclone"],
                kind: EntityKind::Vehicle,
            },
            Self::SizeThreeWeapon => Spec {
                preferred: &[
                    "BEHR_LaserCannon_S3",
                    "KLWE_LaserRepeater_S3",
                    "BEHR_BallisticCannon_S3",
                ],
                prefixes: &[
                    "BEHR_LaserCannon",
                    "KLWE_LaserRepeater",
                    "BEHR_BallisticCannon",
                ],
                kind: EntityKind::Weapon,
            },
        }
    }
}

const fn ship(preferred: &'static [&'static str], prefixes: &'static [&'static str]) -> Spec {
    Spec {
        preferred,
        prefixes,
        kind: EntityKind::Ship,
    }
}
