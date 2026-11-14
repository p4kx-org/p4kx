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
    HierarchicalContextualDataPreset,
    GlobalParams,
    SubHarvestableConfig,
    SubHarvestableMultiConfig,
}

impl LootDefinitionKind {
    pub(super) const ALL: [Self; 8] = [
        Self::PoolFilter,
        Self::SecondaryChoiceSingleLayer,
        Self::SecondaryChoiceMultiLayer,
        Self::Theme,
        Self::HierarchicalContextualDataPreset,
        Self::GlobalParams,
        Self::SubHarvestableConfig,
        Self::SubHarvestableMultiConfig,
    ];

    pub(crate) const fn of(kind: Kind) -> Option<Self> {
        Some(match kind {
            Kind::LootPoolFilters => Self::PoolFilter,
            Kind::LootSecondaryChoiceSingleLayers => Self::SecondaryChoiceSingleLayer,
            Kind::LootSecondaryChoiceMultiLayers => Self::SecondaryChoiceMultiLayer,
            Kind::LootThemes => Self::Theme,
            Kind::HierarchicalContextualDataPresets => Self::HierarchicalContextualDataPreset,
            Kind::LootGenerationGlobalParams => Self::GlobalParams,
            Kind::SubHarvestableConfigs => Self::SubHarvestableConfig,
            Kind::SubHarvestableMultiConfigs => Self::SubHarvestableMultiConfig,
            _ => return None,
        })
    }

    pub(crate) const fn kind(self) -> Kind {
        match self {
            Self::PoolFilter => Kind::LootPoolFilters,
            Self::SecondaryChoiceSingleLayer => Kind::LootSecondaryChoiceSingleLayers,
            Self::SecondaryChoiceMultiLayer => Kind::LootSecondaryChoiceMultiLayers,
            Self::Theme => Kind::LootThemes,
            Self::HierarchicalContextualDataPreset => Kind::HierarchicalContextualDataPresets,
            Self::GlobalParams => Kind::LootGenerationGlobalParams,
            Self::SubHarvestableConfig => Kind::SubHarvestableConfigs,
            Self::SubHarvestableMultiConfig => Kind::SubHarvestableMultiConfigs,
        }
    }

    pub(crate) fn kind_for_type(type_name: &str) -> Option<Kind> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.is_type_wide() && kind.rule().type_name == type_name)
            .map(Self::kind)
    }

    pub(crate) const fn is_type_wide(self) -> bool {
        !matches!(self, Self::PoolFilter)
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
                Self::HierarchicalContextualDataPreset => "HierarchicalContextualDataPresetRecord",
                Self::GlobalParams => "LootGenerationGlobalParams",
                Self::SubHarvestableConfig => "SubHarvestableConfigRecord",
                Self::SubHarvestableMultiConfig => "SubHarvestableMultiConfigRecord",
            },
        }
    }
}
