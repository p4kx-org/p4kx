// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The crafting data of a real Archive through [`read_crafting`]; runs with `--ignored` when
//! `P4KX_DATA_P4K` names one.

use std::error::Error;
use std::time::{Instant, SystemTime};

use p4kx_export::{
    BlueprintTiers, CatalogOptions, Cost, Crafting, ModifierKind, Tier, build_catalog,
    read_crafting,
};
use p4kx_extraction::{Archive, DataForge, DataForgeEntry, Guid, is_placeholder_key};
use p4kx_testkit::real_archive_path;

type TestResult = Result<(), Box<dyn Error>>;

const MIN_BLUEPRINTS: usize = 1_000;
const MIN_POOLS: usize = 100;
const MIN_SOURCES: usize = 1_000;
const CREATURE_DEFAULT: &str = "1dcde125-1275-474f-b0d7-d6682e9e521f";
const PYRO: &str = "286cb603-b4ae-4279-80a1-d4505fee1916";
const LMG_MAGAZINE: &str = "BP_CRAFT_behr_lmg_ballistic_01_mag";
const COVALEX_POOL: &str = "907989eb-9ee8-46a8-b794-032da6bd8a5d";
const COVALEX_BLUEPRINT: &str = "33be7f1a-3f75-4627-bb8b-88e6d6b42f4e";
const MULTIPLIER_BLUEPRINT: &str = "BP_CRAFT_slaver_armor_medium_arms_01_01_01";
const EXPECTED_MULTIPLIER: f32 = 1.15;

fn guid(text: &str) -> Result<Guid, Box<dyn Error>> {
    Ok(text.parse()?)
}

fn costs<'c, 'f>(blueprint: &'c BlueprintTiers<'f>) -> impl Iterator<Item = &'c Cost<'f>> {
    blueprint.tiers.iter().flat_map(Tier::costs)
}

fn multipliers<'f>(crafting: &Crafting<'f>) -> Vec<(&'f str, f32)> {
    let mut found = Vec::new();
    for blueprint in &crafting.blueprints {
        for leaf in costs(blueprint) {
            let multiplier = match leaf {
                Cost::Resource(line) => line.context.quantity_multiplier,
                Cost::Item(line) => line.context.quantity_multiplier,
                _ => None,
            };
            if let Some(multiplier) = multiplier {
                found.push((blueprint.class_name, multiplier));
            }
        }
    }
    found
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn reads_the_crafting_data_of_a_real_archive() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    verify(&entry.open()?)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::print_stdout,
    reason = "the times are the measurement the crafting phase is judged by"
)]
fn times_the_crafting_phase_of_a_real_archive() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let started = Instant::now();
    let crafting = read_crafting(&forge, None, None)?;
    println!(
        "read_crafting: {:?}, {} pools, {} reward sources, warnings {:?}",
        started.elapsed(),
        crafting.pools.len(),
        crafting.reward_sources.len(),
        crafting.warnings
    );
    let started = Instant::now();
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    println!(
        "build_catalog: {:?}, {} bytes",
        started.elapsed(),
        built.json().len()
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::print_stdout,
    reason = "the counts are what the changelog and ADR-0154 state"
)]
fn counts_placeholder_properties_and_pools_without_a_source() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let crafting = read_crafting(&forge, None, None)?;
    let placeholders = crafting
        .properties
        .iter()
        .filter(|property| property.name_key.is_some_and(is_placeholder_key))
        .count();
    let sourceless = crafting
        .pools
        .iter()
        .filter(|pool| {
            !crafting
                .reward_sources
                .iter()
                .any(|source| source.pool == pool.guid)
        })
        .count();
    println!(
        "properties {} of which placeholder name {placeholders}; pools {} without a source {sourceless}",
        crafting.properties.len(),
        crafting.pools.len()
    );
    Ok(())
}

fn verify(forge: &DataForge<'_>) -> TestResult {
    let crafting = read_crafting(forge, None, None)?;
    assert!(crafting.failures.is_empty(), "{:?}", crafting.failures);
    assert!(crafting.blueprints.len() >= MIN_BLUEPRINTS, "blueprints");
    assert!(
        crafting.blueprints.iter().all(|bp| !bp.tiers.is_empty()),
        "every blueprint has a tier"
    );
    assert!(
        crafting.blueprints.is_sorted_by_key(|bp| bp.guid),
        "ordered by GUID"
    );
    check_selections(&crafting)?;
    check_modifiers(&crafting);
    check_distributions(&crafting)?;
    check_rewards(&crafting)?;
    check_dismantle(&crafting)
}

fn check_selections(crafting: &Crafting<'_>) -> TestResult {
    let magazine = crafting
        .blueprints
        .iter()
        .find(|bp| bp.class_name == LMG_MAGAZINE)
        .ok_or("the LMG magazine blueprint")?;
    let tier = magazine.tiers.first().ok_or("a tier")?;
    assert_eq!(tier.craft_time_seconds, Some(10), "craft time");
    let Some(Cost::Select(outer)) = tier.mandatory_cost() else {
        return Err("a selection as mandatory cost".into());
    };
    assert_eq!(outer.count, Some(2), "choose 2 of 2");
    assert_eq!(outer.options.len(), 2, "two slots");
    let fewer = crafting
        .blueprints
        .iter()
        .filter(|bp| {
            costs(bp)
                .any(|cost| matches!(cost, Cost::Select(selection) if selection.chooses_fewer()))
        })
        .count();
    assert!(fewer >= 1, "blueprints that let the player choose fewer");
    let found = multipliers(crafting);
    assert!(
        found
            .iter()
            .any(|(name, multiplier)| *name == MULTIPLIER_BLUEPRINT
                && (multiplier - EXPECTED_MULTIPLIER).abs() < f32::EPSILON),
        "the slaver arms carry the factor 1.15: {found:?}"
    );
    Ok(())
}

fn check_modifiers(crafting: &Crafting<'_>) {
    let mut ranges = Vec::new();
    for blueprint in &crafting.blueprints {
        for cost in costs(blueprint) {
            if let Cost::Select(selection) = cost {
                for modifier in &selection.context.modifiers {
                    ranges.extend(modifier.ranges.iter().copied());
                }
            }
        }
    }
    assert!(
        ranges
            .iter()
            .any(|range| range.kind == ModifierKind::Multiplier
                && range.start_quality == Some(0)
                && range.end_quality == Some(1000)
                && range.at_start.is_some()
                && range.at_end.is_some()),
        "a linear multiplier over the whole quality range"
    );
    assert!(
        ranges
            .iter()
            .any(|range| range.kind == ModifierKind::Additive && range.at_start.is_some()),
        "an additive range"
    );
    assert!(!crafting.properties.is_empty(), "property definitions");
}

fn check_distributions(crafting: &Crafting<'_>) -> TestResult {
    let creature_default = guid(CREATURE_DEFAULT)?;
    let default = crafting
        .quality_distributions
        .iter()
        .find(|item| item.guid == creature_default)
        .ok_or("the default creature distribution")?;
    let curve = default.distribution.as_ref().ok_or("a curve")?;
    assert_eq!(curve.kind, "CraftingQualityDistributionNormal", "kind");
    assert_eq!(
        (curve.min, curve.max, curve.mean, curve.stddev),
        (Some(1), Some(1000), Some(269.0), Some(254.0)),
        "the curve"
    );
    let pyro = guid(PYRO)?;
    assert!(
        crafting
            .quality_distributions
            .iter()
            .flat_map(|item| &item.overrides)
            .any(|over| over.location == Some(pyro) && over.distribution.is_some()),
        "an override for Pyro"
    );
    Ok(())
}

fn check_rewards(crafting: &Crafting<'_>) -> TestResult {
    assert!(crafting.pools.len() >= MIN_POOLS, "pools");
    assert!(crafting.reward_sources.len() >= MIN_SOURCES, "sources");
    let covalex_pool = guid(COVALEX_POOL)?;
    let pool = crafting
        .pools
        .iter()
        .find(|pool| pool.guid == covalex_pool)
        .ok_or("the Covalex pool")?;
    assert!(!pool.entries.is_empty(), "entries");
    #[expect(
        clippy::cast_precision_loss,
        reason = "a pool has far fewer than 2^52 entries"
    )]
    let share = 1.0 / pool.entries.len() as f64;
    let chances = crafting.reward_chances();
    let blueprint = guid(COVALEX_BLUEPRINT)?;
    let from_pool: Vec<f64> = chances
        .get(&blueprint)
        .ok_or("chances for the blueprint")?
        .iter()
        .filter(|chance| chance.pool == pool.guid)
        .map(|chance| chance.chance)
        .collect();
    assert!(!from_pool.is_empty(), "contracts reward it");
    assert!(
        from_pool
            .iter()
            .any(|chance| (chance - share).abs() < 1.0e-6),
        "chance 1 times weight 1 over the pool size: {from_pool:?}"
    );
    Ok(())
}

fn check_dismantle(crafting: &Crafting<'_>) -> TestResult {
    let process = crafting.dismantle.first().ok_or("a dismantle process")?;
    assert_eq!(process.efficiency, Some(0.5), "efficiency");
    assert_eq!(process.time_seconds, Some(15), "time");
    let magazine = crafting
        .blueprints
        .iter()
        .find(|bp| bp.class_name == LMG_MAGAZINE)
        .ok_or("the magazine blueprint")?;
    let link = crafting
        .dismantle_return(magazine)
        .ok_or("a dismantle return for the magazine")?;
    assert_eq!(link.process.guid, process.guid, "the process");
    assert_eq!(Some(link.item), magazine.produced_item, "the item");
    assert_eq!(link.blueprint, magazine.guid, "the blueprint");
    assert!(
        crafting.global.default_blueprints.contains(&process.guid),
        "the dismantle process is a default blueprint"
    );
    assert!(
        !crafting.global.dismantle_blacklist_resources.is_empty(),
        "blacklisted resources"
    );
    Ok(())
}
