// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

/// Something that makes the crafting data incomplete or ambiguous without failing a Record.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CraftingWarning {
    /// This many Records could not be read although their struct type is that of a Record with a
    /// reward source; a source in one of them is missing from
    /// [`Crafting::reward_sources`](crate::Crafting::reward_sources). A Record of a type that
    /// holds no source elsewhere is not counted.
    UnreadableRewardRecords(usize),
    /// This many modifier ranges of blueprints have a struct type the reader does not know, so
    /// they are missing from the modifiers of [`Crafting::blueprints`](crate::Crafting::blueprints).
    /// `details` names the types, ordered and without repeats.
    UnknownModifierRanges {
        /// The number of ranges.
        count: usize,
        /// One entry per struct type.
        details: Vec<String>,
    },
    /// The DataForge holds this many dismantle processes instead of one, so
    /// [`Crafting::dismantle_return`](crate::Crafting::dismantle_return) links no blueprint.
    AmbiguousDismantle(usize),
    /// This many ships have a vehicle implementation file or Modification patch file that exists
    /// but cannot be read, so no paint is assigned through it. `details` names the files and why
    /// they cannot be read, ordered and without repeats. A file the Archive lacks is no such ship.
    UnreadableVehicleFiles {
        /// The number of ships.
        count: usize,
        /// One entry per file: its path and the reason.
        details: Vec<String>,
    },
    /// This many references to containers that cannot be read, Child elements with a malformed or
    /// out-of-range placement and walk limits were hit, so the starmap objects below them have no
    /// position. A container the Archive lacks is not counted.
    UnplacedStarmapContainers(usize),
    /// This many system containers cannot be opened or their `.soc` cannot be read, so a
    /// harvestable provider preset they name is missing from `harvestableLocations`. `details`
    /// names the containers and why they cannot be read, ordered.
    UnreadableHarvestContainers {
        /// The number of containers.
        count: usize,
        /// One entry per container: its path and the reason.
        details: Vec<String>,
    },
    /// This many harvestable locations have no starmap object although one is recorded above
    /// their container: it is not in the starmap objects, 64 or more mining claim markers lie
    /// above, or a marker has no object above it. A container with no object recorded above is
    /// the normal case and not counted. `details` gives each reason with its number, ordered.
    HarvestLocationsWithoutObject {
        /// The number of locations.
        count: usize,
        /// One entry per reason: the number and the reason.
        details: Vec<String>,
    },
    /// This many containers or their entity files cannot be read, so a multi config they carry
    /// may be missing from `lootContainers`. `details` names the containers and why, ordered.
    UnreadableLootContainers {
        /// The number of containers.
        count: usize,
        /// One entry per container: its path and the reason.
        details: Vec<String>,
    },
    /// This many entity files of containers name several different containers or multi configs;
    /// only the first of each, in document order, is used. `details` names the files, ordered.
    AmbiguousLootEntityFiles {
        /// The number of entity files.
        count: usize,
        /// One entry per file: its container and path.
        details: Vec<String>,
    },
    /// This many `HarvestablePreset` or entity Records that provider presets and multi configs
    /// name are missing or cannot be built, so the harvestables that name them lack their entity
    /// and composition. `details` names the Records and why, ordered.
    UnreadableHarvestables {
        /// The number of Records.
        count: usize,
        /// One entry per Record: its role, GUID and the reason.
        details: Vec<String>,
    },
    /// The shop registry exists but cannot be read, so `shops` is empty although the Build has
    /// shops.
    UnreadableShopRegistry,
    /// The Archive holds starmap objects but no shop registry at the known path, so `shops` is
    /// empty; the registry may have moved in this Build.
    MissingShopRegistry,
    /// This many shop containers cannot be opened or their `entitycomponentshop.xml` cannot be
    /// read, so the shops they hold may lack kind and brand.
    UnreadableShopContainers(usize),
    /// This many inventory files that shop containers name cannot be read, so the shops that use
    /// them have no item list. `details` names the files and why they cannot be read, ordered and
    /// without repeats.
    UnreadableShopInventories {
        /// The number of files.
        count: usize,
        /// One entry per file: its path and the reason.
        details: Vec<String>,
    },
    /// This many distinct inventory files that shop containers name are not in the Archive, so
    /// the shops that use them have no item list; the files may have moved in this Build.
    MissingShopInventories(usize),
    /// Shop containers name shops, but no `.soc` holds a shop component, so no shop has an item
    /// list; the component layout may have changed in this Build.
    NoShopComponents,
    /// The radar parameters do not name the slot of the scan signature, so `scanSignature` of
    /// `mineableEntities` is `null` everywhere; the text says why. `signatures` is still exported.
    UnknownScanSignatureSlot(&'static str),
    /// This many stored names in the contract templates and mission broker entries (a state, a
    /// category, a comms timing or reason, a kind of objective property, a comparison, a month, a
    /// repeat type, a reaction, a licence type) are not known to the catalog, so they are `null`.
    /// `details` gives each kind and value with its number, ordered.
    UnknownTemplateValues {
        /// The number of names.
        count: usize,
        /// One entry per kind and value: the kind, the value and the number.
        details: Vec<String>,
    },
}

impl CraftingWarning {
    /// Returns the name of the kind of the warning, as `p4kx catalog --json` states it.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::UnreadableRewardRecords(_) => "unreadable_reward_records",
            Self::UnknownModifierRanges { .. } => "unknown_modifier_ranges",
            Self::AmbiguousDismantle(_) => "ambiguous_dismantle",
            Self::UnreadableVehicleFiles { .. } => "unreadable_vehicle_files",
            Self::UnplacedStarmapContainers(_) => "unplaced_starmap_containers",
            Self::UnreadableHarvestContainers { .. } => "unreadable_harvest_containers",
            Self::HarvestLocationsWithoutObject { .. } => "harvest_locations_without_object",
            Self::UnreadableLootContainers { .. } => "unreadable_loot_containers",
            Self::AmbiguousLootEntityFiles { .. } => "ambiguous_loot_entity_files",
            Self::UnreadableHarvestables { .. } => "unreadable_harvestables",
            Self::UnreadableShopRegistry => "unreadable_shop_registry",
            Self::MissingShopRegistry => "missing_shop_registry",
            Self::UnreadableShopContainers(_) => "unreadable_shop_containers",
            Self::UnreadableShopInventories { .. } => "unreadable_shop_inventories",
            Self::MissingShopInventories(_) => "missing_shop_inventories",
            Self::NoShopComponents => "no_shop_components",
            Self::UnknownScanSignatureSlot(_) => "unknown_scan_signature_slot",
            Self::UnknownTemplateValues { .. } => "unknown_template_values",
        }
    }

    /// Returns the number the warning counts, `None` for one that counts nothing.
    #[must_use]
    pub const fn count(&self) -> Option<usize> {
        match self {
            Self::UnreadableRewardRecords(count)
            | Self::AmbiguousDismantle(count)
            | Self::UnplacedStarmapContainers(count)
            | Self::UnreadableShopContainers(count)
            | Self::MissingShopInventories(count)
            | Self::UnknownModifierRanges { count, .. }
            | Self::UnreadableHarvestContainers { count, .. }
            | Self::HarvestLocationsWithoutObject { count, .. }
            | Self::UnreadableLootContainers { count, .. }
            | Self::AmbiguousLootEntityFiles { count, .. }
            | Self::UnreadableHarvestables { count, .. }
            | Self::UnreadableVehicleFiles { count, .. }
            | Self::UnknownTemplateValues { count, .. }
            | Self::UnreadableShopInventories { count, .. } => Some(*count),
            Self::UnreadableShopRegistry
            | Self::MissingShopRegistry
            | Self::NoShopComponents
            | Self::UnknownScanSignatureSlot(_) => None,
        }
    }

    /// Returns the files the warning names and why they cannot be read, empty for a warning
    /// without.
    #[must_use]
    pub fn details(&self) -> &[String] {
        match self {
            Self::UnknownModifierRanges { details, .. }
            | Self::UnreadableHarvestContainers { details, .. }
            | Self::HarvestLocationsWithoutObject { details, .. }
            | Self::UnreadableLootContainers { details, .. }
            | Self::AmbiguousLootEntityFiles { details, .. }
            | Self::UnreadableHarvestables { details, .. }
            | Self::UnreadableVehicleFiles { details, .. }
            | Self::UnknownTemplateValues { details, .. }
            | Self::UnreadableShopInventories { details, .. } => details,
            _ => &[],
        }
    }
}

const LISTED_DETAILS: usize = 8;

/// The details of a warning as ` (a; b)`, empty for none; lists at most [`LISTED_DETAILS`].
struct Listed<'a>(&'a [String]);

impl fmt::Display for Listed<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(first) = self.0.first() else {
            return Ok(());
        };
        write!(f, " ({first}")?;
        for detail in self.0.iter().take(LISTED_DETAILS).skip(1) {
            write!(f, "; {detail}")?;
        }
        match self.0.len().checked_sub(LISTED_DETAILS) {
            Some(more) if more > 0 => write!(f, "; and {more} more)"),
            _ => f.write_str(")"),
        }
    }
}

impl fmt::Display for CraftingWarning {
    #[expect(clippy::too_many_lines, reason = "one arm per warning")]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnreadableRewardRecords(count) => write!(
                f,
                "{count} Records that could hold a reward source cannot be read; their reward sources are missing"
            ),
            Self::UnknownModifierRanges { count, details } => write!(
                f,
                "{count} modifier ranges have a type P4KX does not know; they are missing from the blueprint modifiers{}",
                Listed(details)
            ),
            Self::AmbiguousDismantle(count) => write!(
                f,
                "the DataForge holds {count} dismantle processes instead of one; no dismantle return is linked"
            ),
            Self::UnreadableVehicleFiles { count, details } => write!(
                f,
                "{count} ships have a vehicle file that cannot be read; their paints are missing{}",
                Listed(details)
            ),
            Self::UnplacedStarmapContainers(count) => write!(
                f,
                "{count} system containers or placements cannot be read; the starmap objects below them have no position"
            ),
            Self::UnreadableHarvestContainers { count, details } => write!(
                f,
                "{count} system containers cannot be read for a provider preset; the preset may be missing from the harvestable locations{}",
                Listed(details)
            ),
            Self::HarvestLocationsWithoutObject { count, details } => write!(
                f,
                "{count} harvestable locations have no starmap object although one is recorded above their container{}",
                Listed(details)
            ),
            Self::UnreadableLootContainers { count, details } => write!(
                f,
                "{count} containers cannot be read for a multi config; the config may be missing from the loot containers{}",
                Listed(details)
            ),
            Self::AmbiguousLootEntityFiles { count, details } => write!(
                f,
                "{count} entity files name several containers or multi configs; only the first of each is used{}",
                Listed(details)
            ),
            Self::UnreadableHarvestables { count, details } => write!(
                f,
                "{count} harvestable Records cannot be read; the harvestables that name them lack their entity and composition{}",
                Listed(details)
            ),
            Self::UnreadableShopRegistry => f.write_str(
                "the shop registry cannot be read; the catalog has no shops although the Build has some",
            ),
            Self::MissingShopRegistry => f.write_str(
                "the Archive holds no shop registry at the known path; the catalog has no shops",
            ),
            Self::UnreadableShopContainers(count) => write!(
                f,
                "{count} shop containers cannot be read; the shops in them may lack kind and brand"
            ),
            Self::UnreadableShopInventories { count, details } => write!(
                f,
                "{count} shop inventory files cannot be read; the shops that use them have no item list{}",
                Listed(details)
            ),
            Self::MissingShopInventories(count) => write!(
                f,
                "{count} shop inventory files that shop containers name are not in the Archive; the shops that use them have no item list"
            ),
            Self::UnknownTemplateValues { count, details } => write!(
                f,
                "{count} stored names in the contract templates and mission broker entries are not known; they are null{}",
                Listed(details)
            ),
            Self::UnknownScanSignatureSlot(reason) => write!(
                f,
                "{reason}; the scan signature of the mineable entities is null"
            ),
            Self::NoShopComponents => f.write_str(
                "no shop component was found in the .soc files of the shop containers; no shop has an item list",
            ),
        }
    }
}
