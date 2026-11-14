// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Kind;
use crate::selection::KindRule;

/// Exact source Record families with owner-local Loot value arenas.
#[derive(Clone, Copy)]
pub(crate) enum LootDefinitionKind {
    PoolFilter,
    SecondaryChoiceSingleLayer,
    SecondaryChoiceMultiLayer,
    Theme,
}

impl LootDefinitionKind {
    pub(super) const ALL: [Self; 4] = [
        Self::PoolFilter,
        Self::SecondaryChoiceSingleLayer,
        Self::SecondaryChoiceMultiLayer,
        Self::Theme,
    ];

    pub(crate) const fn of(kind: Kind) -> Option<Self> {
        Some(match kind {
            Kind::LootPoolFilters => Self::PoolFilter,
            Kind::LootSecondaryChoiceSingleLayers => Self::SecondaryChoiceSingleLayer,
            Kind::LootSecondaryChoiceMultiLayers => Self::SecondaryChoiceMultiLayer,
            Kind::LootThemes => Self::Theme,
            _ => return None,
        })
    }

    pub(crate) const fn kind(self) -> Kind {
        match self {
            Self::PoolFilter => Kind::LootPoolFilters,
            Self::SecondaryChoiceSingleLayer => Kind::LootSecondaryChoiceSingleLayers,
            Self::SecondaryChoiceMultiLayer => Kind::LootSecondaryChoiceMultiLayers,
            Self::Theme => Kind::LootThemes,
        }
    }

    pub(crate) const fn rule(self) -> KindRule {
        KindRule {
            prefixes: match self {
                Self::PoolFilter => &["libs/foundry/records/lootgeneration/filters/"],
                _ => &[""],
            },
            type_name: match self {
                Self::PoolFilter => "PoolFilterRecord",
                Self::SecondaryChoiceSingleLayer => "LootV3SecondaryChoicesSingleLayerRecord",
                Self::SecondaryChoiceMultiLayer => "LootV3SecondaryChoicesMultiLayerRecord",
                Self::Theme => "LootThemeRecord",
            },
        }
    }
}
