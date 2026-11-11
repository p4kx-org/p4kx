// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use crate::crafting_research::Research;

pub use crate::quality_curve::QualityCurve;

/// How the quality range of a gameplay-property modifier changes the property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModifierKind {
    /// A factor: `CraftingGameplayPropertyModifierValueRange_Linear`.
    Multiplier,
    /// A summand: `CraftingGameplayPropertyModifierValueRange_LinearIntegerAdditive`.
    Additive,
}

/// One linear range of a modifier: the value at the quality `start_quality` and at `end_quality`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ValueRange {
    /// Whether the values are factors or summands.
    pub kind: ModifierKind,
    /// The quality the range starts at, `None` when the Record holds none.
    pub start_quality: Option<i64>,
    /// The quality the range ends at.
    pub end_quality: Option<i64>,
    /// The modifier at the start quality.
    pub at_start: Option<f32>,
    /// The modifier at the end quality.
    pub at_end: Option<f32>,
}

/// A gameplay property of the crafted item with the ranges that scale it by quality.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyModifier {
    /// The GUID of the `CraftingGameplayPropertyDef` Record, `None` for a null Reference.
    pub property: Option<Guid>,
    /// The ranges in stored order.
    pub ranges: Vec<ValueRange>,
}

/// The context of a cost: what the game applies to it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CostContext<'f> {
    /// The `CraftingCostContext_QuantityMultiplier` factor, `None` without one.
    pub quantity_multiplier: Option<f32>,
    /// The option of `CraftingCostContext_ResultCompositionInclusion`, e.g. `Include`.
    pub composition_inclusion: Option<&'f str>,
    /// The modifiers of `CraftingCostContext_ResultGameplayPropertyModifiers`.
    pub modifiers: Vec<PropertyModifier>,
}

/// A resource line: a quantity of a `ResourceType`.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceCost<'f> {
    /// The GUID of the `ResourceType` Record.
    pub resource: Option<Guid>,
    /// The quantity in standard cargo units (SCU), before the quantity multiplier.
    pub quantity_scu: Option<f32>,
    /// The minimum quality of the resource.
    pub min_quality: Option<i64>,
    /// What the game applies to the line.
    pub context: CostContext<'f>,
}

/// An item line: a number of entities.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemCost<'f> {
    /// The GUID of the entity.
    pub item: Option<Guid>,
    /// The number of units, before the quantity multiplier.
    pub quantity: Option<i64>,
    /// The minimum quality of the item.
    pub min_quality: Option<i64>,
    /// What the game applies to the line.
    pub context: CostContext<'f>,
}

/// A selection ("choose n"): the player picks `count` of its options.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection<'f> {
    /// How many options the player chooses, `None` when the Record holds no count.
    pub count: Option<i64>,
    /// The debug name of the slot, `None` without one.
    pub slot: Option<&'f str>,
    /// The localisation key of the slot name, `None` without one or for a placeholder.
    pub slot_key: Option<&'f str>,
    /// What the game applies to the selection.
    pub context: CostContext<'f>,
    /// The option indices in stored order in this tier.
    pub options: Vec<CostIndex>,
}

impl Selection<'_> {
    /// Returns whether the player may choose fewer options than the selection offers.
    #[must_use]
    pub fn chooses_fewer(&self) -> bool {
        self.count
            .and_then(|count| usize::try_from(count).ok())
            .is_some_and(|count| count < self.options.len())
    }
}

/// One cost of a recipe.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Cost<'f> {
    /// A selection of costs.
    Select(Selection<'f>),
    /// A resource line.
    Resource(ResourceCost<'f>),
    /// An item line.
    Item(ItemCost<'f>),
}

/// One cost position in its owning tier's arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostIndex(usize);

impl CostIndex {
    pub(crate) const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the position in the owning tier's cost slice.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// One tier of a blueprint: its craft time and its costs.
#[derive(Debug, Clone, PartialEq)]
pub struct Tier<'f> {
    /// The craft time in whole seconds, `None` without one.
    pub craft_time_seconds: Option<i64>,
    /// The mandatory cost position, `None` without one.
    pub mandatory: Option<CostIndex>,
    /// The optional cost positions in stored order.
    pub optional: Vec<CostIndex>,
    /// The actual research envelope and member states.
    pub research: Research<'f>,
    costs: Vec<Cost<'f>>,
}

impl<'f> Tier<'f> {
    pub(crate) const fn new(
        craft_time_seconds: Option<i64>,
        mandatory: Option<CostIndex>,
        optional: Vec<CostIndex>,
        costs: Vec<Cost<'f>>,
        research: Research<'f>,
    ) -> Self {
        Self {
            craft_time_seconds,
            mandatory,
            optional,
            research,
            costs,
        }
    }

    /// Returns all cost nodes in original depth-first order.
    #[must_use]
    pub const fn costs(&self) -> &[Cost<'f>] {
        self.costs.as_slice()
    }

    /// Borrows a cost by its position in this tier.
    #[must_use]
    pub fn cost(&self, index: CostIndex) -> Option<&Cost<'f>> {
        self.costs.get(index.index())
    }

    /// Borrows the mandatory cost when the tier has one.
    #[must_use]
    pub fn mandatory_cost(&self) -> Option<&Cost<'f>> {
        self.mandatory.and_then(|index| self.cost(index))
    }
}

/// A blueprint with all its tiers.
#[derive(Debug, Clone, PartialEq)]
pub struct BlueprintTiers<'f> {
    /// The GUID of the `CraftingBlueprintRecord`.
    pub guid: Guid,
    /// The Record name without its type.
    pub class_name: &'f str,
    /// The record path, without `.xml`.
    pub record_path: &'f str,
    /// The localisation key of the blueprint name, a placeholder key included, `None` without one.
    pub name_key: Option<&'f str>,
    /// The GUID of the `BlueprintCategoryRecord` the blueprint belongs to, `None` without one.
    pub category: Option<Guid>,
    /// The GUID of the entity the blueprint produces, `None` without one.
    pub produced_item: Option<Guid>,
    /// The tiers in stored order, the first one being the tier the catalog's `ingredients`
    /// describe.
    pub tiers: Vec<Tier<'f>>,
    /// The struct type of every modifier range of the blueprint that the reader does not know,
    /// one entry per range in stored order. Such a range is not in any [`PropertyModifier`].
    pub unknown_range_types: Vec<&'f str>,
}

/// A gameplay property definition: a `CraftingGameplayPropertyDef` Record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameplayProperty<'f> {
    /// The GUID of the Record.
    pub guid: Guid,
    /// The Record name without its type.
    pub class_name: &'f str,
    /// The record path, without `.xml`.
    pub record_path: &'f str,
    /// The localisation key of the property name, `None` for none or a placeholder.
    pub name_key: Option<&'f str>,
}

/// A distribution that replaces the default one at a location.
#[derive(Debug, Clone, PartialEq)]
pub struct LocationOverride<'f> {
    /// The GUID of the location Record, `None` for a null Reference.
    pub location: Option<Guid>,
    /// The distribution at the location, `None` when the Record holds none.
    pub distribution: Option<QualityCurve<'f>>,
}

/// A quality distribution Record: the default distribution of a source or its location
/// overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct QualityDistribution<'f> {
    /// The GUID of the Record.
    pub guid: Guid,
    /// The Record name without its type.
    pub class_name: &'f str,
    /// The record path, without `.xml`.
    pub record_path: &'f str,
    /// The distribution of a `CraftingQualityDistributionRecord`, `None` for an override Record.
    pub distribution: Option<QualityCurve<'f>>,
    /// The overrides of a `CraftingQualityLocationOverrideRecord`, empty for a default Record.
    pub overrides: Vec<LocationOverride<'f>>,
}

/// One band of a quality quantization: the qualities `start` to `end` count as `mapped_value`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantizationBand {
    /// The lowest quality of the band.
    pub start: i64,
    /// The highest quality of the band, inclusive.
    pub end: i64,
    /// The quality the game uses for every quality in the band.
    pub mapped_value: i64,
}

/// A quality quantization: a `CraftingQualityQuantizationRecord`, the quality steps of the
/// `ResourceType`s that link to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityQuantization<'f> {
    /// The GUID of the Record.
    pub guid: Guid,
    /// The Record name without its type.
    pub class_name: &'f str,
    /// The record path, without `.xml`.
    pub record_path: &'f str,
    /// The bands in stored order.
    pub bands: Vec<QuantizationBand>,
    /// The GUIDs of the `ResourceType` Records whose crafting data link to the Record, ordered and
    /// without repeats; empty for a Record no `ResourceType` uses.
    pub resource_types: Vec<Guid>,
}

/// One entry of a blueprint pool.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoolEntry {
    /// The GUID of the blueprint, `None` for a null Reference.
    pub blueprint: Option<Guid>,
    /// The weight of the entry among the pool's.
    pub weight: Option<f32>,
}

/// A blueprint pool: a `BlueprintPoolRecord` a reward draws a blueprint from.
#[derive(Debug, Clone, PartialEq)]
pub struct BlueprintPool<'f> {
    /// The GUID of the Record.
    pub guid: Guid,
    /// The Record name without its type.
    pub class_name: &'f str,
    /// The record path, without `.xml`.
    pub record_path: &'f str,
    /// The entries in stored order.
    pub entries: Vec<PoolEntry>,
}

impl BlueprintPool<'_> {
    /// Returns the sum of the positive weights of the entries.
    #[must_use]
    pub fn total_weight(&self) -> f32 {
        self.entries
            .iter()
            .filter_map(|entry| entry.weight)
            .filter(|weight| *weight > 0.0)
            .sum()
    }
}

/// A place that rewards a blueprint of a pool with a chance: a `BlueprintRewards` result of a
/// contract.
#[derive(Debug, Clone, PartialEq)]
pub struct RewardSource<'f> {
    /// The GUID of the Record that holds the reward.
    pub record: Guid,
    /// The record path of that Record, without `.xml`.
    pub record_path: &'f str,
    /// The `id` of the nearest enclosing struct that has one, e.g. the contract.
    pub contract_id: Option<Guid>,
    /// The `debugName` paired with the nearest enclosing `id`, or the nearest name without an id.
    pub contract_name: Option<&'f str>,
    /// The GUID of the `BlueprintPoolRecord` the Reference links to.
    pub pool: Guid,
    /// The chance that the reward is given as the Record states it, 0 to 1 on LIVE and not
    /// clamped, `None` when the Record holds none.
    pub chance: Option<f32>,
}

/// The chance that one reward source gives a blueprint: the chance of the source times the
/// weight of the blueprint over the sum of the weights of its pool.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RewardChance<'f> {
    /// The GUID of the Record that holds the reward.
    pub record: Guid,
    /// The record path of that Record, without `.xml`.
    pub record_path: &'f str,
    /// The `id` of the contract, if any.
    pub contract_id: Option<Guid>,
    /// The `debugName` of the contract, if any.
    pub contract_name: Option<&'f str>,
    /// The GUID of the pool.
    pub pool: Guid,
    /// The chance that the source gives the blueprint, 0 to 1.
    pub chance: f64,
}

/// A generic dismantle process: a `CraftingBlueprintRecord` under `crafting/blueprints/dismantle`
/// whose process is a `GenericCraftingProcess_Dismantle`.
#[derive(Debug, Clone, PartialEq)]
pub struct DismantleProcess<'f> {
    /// The GUID of the Record.
    pub guid: Guid,
    /// The Record name without its type.
    pub class_name: &'f str,
    /// The record path, without `.xml`.
    pub record_path: &'f str,
    /// The share of the crafting ingredients a dismantled item returns, 0 to 1.
    pub efficiency: Option<f32>,
    /// The dismantle time in whole seconds.
    pub time_seconds: Option<i64>,
}

/// The `CraftingGlobalParams` Record: the default blueprints and the dismantle exclusions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalParams {
    /// The blueprints a player has by default (`defaultBlueprintSelection`).
    pub default_blueprints: Vec<Guid>,
    /// The `ResourceType` Records dismantling never returns.
    pub dismantle_blacklist_resources: Vec<Guid>,
    /// The entities that cannot be dismantled.
    pub dismantle_blacklist_items: Vec<Guid>,
}

/// The dismantle return of a blueprint's item.
///
/// The item returns the share `process.efficiency` of the costs of the tier `tier` of the
/// blueprint, except the resources in `excluded_resources`. These are assumptions of the
/// library, not verified against the game:
///
/// - Only the first tier counts (the only one on LIVE); `tier` is `None` for a blueprint
///   without a tier.
/// - A [`Cost::Select`] is not resolved: the Records do not say which options a dismantle
///   returns, so the consumer applies the count itself.
/// - The quantity multiplier of a cost line is not applied; the quantities are the stored ones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DismantleReturn<'c, 'f> {
    /// The only dismantle process of the DataForge.
    pub process: &'c DismantleProcess<'f>,
    /// The GUID of the blueprint whose costs the item returns by `process.efficiency`.
    pub blueprint: Guid,
    /// The GUID of the item.
    pub item: Guid,
    /// The index of the tier whose costs are returned, `None` for a blueprint without a tier.
    pub tier: Option<usize>,
    /// The `ResourceType` Records a dismantle never returns.
    pub excluded_resources: &'c [Guid],
}

impl DismantleReturn<'_, '_> {
    /// Returns whether a dismantle may return the `ResourceType` `resource`.
    #[must_use]
    pub fn returns_resource(&self, resource: &Guid) -> bool {
        !self.excluded_resources.contains(resource)
    }
}
