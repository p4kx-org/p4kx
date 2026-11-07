// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The crafting data of a synthetic DataForge through [`read_crafting`].

use crate::crafting_detail_fixture;

use std::error::Error;

use crafting_detail_fixture as fixture;
use p4kx_export::{
    CancelToken, CatalogError, Cost, Crafting, CraftingWarning, Filter, ModifierKind, Selection,
    read_crafting,
};
use p4kx_extraction::{DataForge, Guid};

type TestResult = Result<(), Box<dyn Error>>;

pub(crate) fn guid(value: u128) -> Guid {
    let text = format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        value >> 96,
        (value >> 80) & 0xffff,
        (value >> 64) & 0xffff,
        (value >> 48) & 0xffff,
        value & 0xffff_ffff_ffff
    );
    text.parse().unwrap_or_default()
}

fn read<R>(
    filter: Option<&Filter>,
    check: impl FnOnce(Crafting<'_>) -> Result<R, Box<dyn Error>>,
) -> Result<R, Box<dyn Error>> {
    let bytes = fixture::database().build();
    let forge = DataForge::open(&bytes)?;
    check(read_crafting(&forge, filter, None)?)
}

fn select<'c, 'f>(cost: Option<&'c Cost<'f>>) -> Result<&'c Selection<'f>, Box<dyn Error>> {
    match cost {
        Some(Cost::Select(selection)) => Ok(selection),
        _ => Err("a selection".into()),
    }
}

#[test]
fn a_blueprint_keeps_every_tier_with_its_craft_time() -> TestResult {
    read(None, |crafting| {
        let classes: Vec<&str> = crafting.blueprints.iter().map(|b| b.class_name).collect();
        assert_eq!(
            classes,
            ["BP_CRAFT_two_tiers", "BP_CRAFT_pick", "BP_CRAFT_bare"],
            "ordered by GUID, the development leftover left out"
        );
        assert_eq!(crafting.skipped_blueprints, 1, "the leftover is counted");
        let blueprint = &crafting.blueprints[0];
        assert_eq!(blueprint.guid, guid(fixture::TWO_TIERS));
        assert_eq!(blueprint.produced_item, Some(guid(fixture::PRODUCT_A)));
        let times: Vec<Option<i64>> = blueprint
            .tiers
            .iter()
            .map(|tier| tier.craft_time_seconds)
            .collect();
        assert_eq!(times, [Some(90), Some(120)], "both tiers");
        assert!(crafting.blueprints[2].tiers.is_empty(), "no tier");
        Ok(())
    })
}

#[test]
fn a_selection_keeps_its_count_and_options() -> TestResult {
    read(None, |crafting| {
        let first = &crafting.blueprints[0].tiers[0];
        let outer = select(first.mandatory.as_ref())?;
        assert_eq!(outer.count, Some(2), "choose 2");
        assert_eq!(outer.slot, Some("ASPECTS"));
        assert_eq!(outer.slot_key, Some("@crafting_slot"));
        assert_eq!(outer.options.len(), 2);
        assert!(!outer.chooses_fewer(), "choose all offered");
        let pick = select(crafting.blueprints[1].tiers[0].mandatory.as_ref())?;
        assert_eq!(
            (pick.count, pick.options.len()),
            (Some(1), 3),
            "choose 1 of 3"
        );
        assert!(pick.chooses_fewer());
        let Some(Cost::Item(line)) = first.optional.first() else {
            return Err("an optional item line".into());
        };
        assert_eq!(
            (line.item, line.quantity),
            (Some(guid(fixture::PRODUCT_B)), Some(3))
        );
        Ok(())
    })
}

#[test]
fn a_cost_line_keeps_its_quantity_multiplier_and_inclusion() -> TestResult {
    read(None, |crafting| {
        let outer = select(crafting.blueprints[0].tiers[0].mandatory.as_ref())?;
        let frame = select(outer.options.first())?;
        let Some(Cost::Resource(line)) = frame.options.first() else {
            return Err("a resource line".into());
        };
        assert_eq!(line.context.quantity_multiplier, Some(1.15));
        assert_eq!(line.quantity_scu, Some(0.5), "the stored quantity");
        assert_eq!(line.min_quality, Some(300));
        assert_eq!(line.resource, Some(guid(fixture::IRON)));
        let core = select(outer.options.get(1))?;
        let Some(Cost::Resource(line)) = core.options.first() else {
            return Err("a resource line".into());
        };
        assert_eq!(line.context.quantity_multiplier, None);
        assert_eq!(line.context.composition_inclusion, Some("Include"));
        Ok(())
    })
}

#[test]
fn property_modifiers_keep_their_quality_ranges() -> TestResult {
    read(None, |crafting| {
        let second = &crafting.blueprints[0].tiers[1];
        let slot = select(second.mandatory.as_ref())?;
        let Some(Cost::Resource(line)) = slot.options.first() else {
            return Err("a resource line".into());
        };
        let [modifier] = line.context.modifiers.as_slice() else {
            return Err("one modifier".into());
        };
        assert_eq!(modifier.property, Some(guid(fixture::PROPERTY)));
        let [linear, additive] = modifier.ranges.as_slice() else {
            return Err("two ranges".into());
        };
        assert_eq!(linear.kind, ModifierKind::Multiplier);
        assert_eq!(
            (linear.start_quality, linear.end_quality),
            (Some(0), Some(1000))
        );
        assert_eq!((linear.at_start, linear.at_end), (Some(0.85), Some(1.15)));
        assert_eq!(additive.kind, ModifierKind::Additive);
        assert_eq!(
            (additive.end_quality, additive.at_start, additive.at_end),
            (Some(249), Some(-2.0), Some(-1.0))
        );
        let [property] = crafting.properties.as_slice() else {
            return Err("one property".into());
        };
        assert_eq!(property.guid, guid(fixture::PROPERTY));
        assert_eq!(property.name_key, Some("@StatName_GPP_Armor"));
        Ok(())
    })
}

#[test]
fn quality_distributions_keep_their_location_overrides() -> TestResult {
    read(None, |crafting| {
        let [default, over] = crafting.quality_distributions.as_slice() else {
            return Err("two distribution Records".into());
        };
        assert_eq!(default.guid, guid(fixture::DEFAULT_CURVE));
        let curve = default.distribution.as_ref().ok_or("a curve")?;
        assert_eq!(curve.kind, "CraftingQualityDistributionNormal");
        assert_eq!(
            (curve.min, curve.max, curve.mean, curve.stddev),
            (Some(1), Some(1000), Some(269.0), Some(254.0))
        );
        assert_eq!(default.overrides, [], "a default Record has no override");
        assert_eq!(over.guid, guid(fixture::OVERRIDE_CURVE));
        assert!(over.distribution.is_none());
        let [entry] = over.overrides.as_slice() else {
            return Err("one override".into());
        };
        assert_eq!(entry.location, Some(guid(fixture::PYRO)));
        assert_eq!(
            entry.distribution.as_ref().map(|c| c.stddev),
            Some(Some(1.0)),
            "the finite override standard deviation"
        );
        assert_eq!(
            entry.distribution.as_ref().map(|c| c.mean),
            Some(Some(295.0))
        );
        Ok(())
    })
}

#[test]
fn reward_sources_carry_their_chance_and_the_blueprint_its_share_of_the_pool() -> TestResult {
    read(None, |crafting| {
        let pool = crafting
            .pools
            .iter()
            .find(|pool| pool.guid == guid(fixture::POOL))
            .ok_or("the pool")?;
        assert_eq!(pool.total_weight(), 4.0);
        let [first, second, _, _, nameless] = crafting.reward_sources.as_slice() else {
            return Err("five reward sources".into());
        };
        assert_eq!(first.record, guid(fixture::CONTRACTS));
        assert_eq!(first.contract_id, Some(guid(fixture::CONTRACT_ID)));
        assert_eq!(first.contract_name, Some("Haul_Intro"));
        assert_eq!((first.pool, first.chance), (guid(fixture::POOL), Some(0.5)));
        assert_eq!(second.pool, guid(fixture::EMPTY_POOL));
        assert_eq!(
            (nameless.contract_id, nameless.contract_name),
            (Some(guid(fixture::NAMELESS_CONTRACT_ID)), None),
            "the id and the name come from the same struct: a contract without a name does not \
             take the name of the generator above it"
        );
        let chances = crafting.reward_chances();
        let one = chances.get(&guid(fixture::TWO_TIERS)).ok_or("chances")?;
        let values: Vec<f64> = one.iter().map(|chance| chance.chance).collect();
        assert_eq!(
            values.len(),
            2,
            "the empty pool and the source with chance 1.5 give none: {values:?}"
        );
        assert!((values[0] - 0.125).abs() < 1.0e-9, "0.5 x 1 / 4");
        assert!(
            (values[1] - 0.5).abs() < 1.0e-9,
            "the negative weight is left out of the total: 0.5 x 2 / 2"
        );
        assert_eq!(one[0].contract_name, Some("Haul_Intro"));
        assert_eq!(
            one[0].record_path,
            "libs/foundry/records/contracts/contractgenerator/haul"
        );
        let other = chances.get(&guid(fixture::CHOOSE_ONE)).ok_or("chances")?;
        assert!((other[0].chance - 0.375).abs() < 1.0e-9, "0.5 x 3 / 4");
        assert!(!chances.contains_key(&guid(fixture::BARE)), "in no pool");
        Ok(())
    })
}

#[test]
fn dismantle_returns_link_the_process_to_the_item_and_blueprint() -> TestResult {
    read(None, |crafting| {
        let [process] = crafting.dismantle.as_slice() else {
            return Err("one process".into());
        };
        assert_eq!(process.guid, guid(fixture::DISMANTLE));
        assert_eq!(process.efficiency, Some(0.5));
        assert_eq!(process.time_seconds, Some(15));
        let link = crafting
            .dismantle_return(&crafting.blueprints[0])
            .ok_or("a return")?;
        assert_eq!(link.process.guid, process.guid);
        assert_eq!(link.blueprint, guid(fixture::TWO_TIERS));
        assert_eq!(link.item, guid(fixture::PRODUCT_A));
        assert_eq!(link.tier, Some(0), "the first tier counts");
        assert_eq!(link.excluded_resources, [guid(fixture::GOLD)]);
        assert!(!link.returns_resource(&guid(fixture::GOLD)), "excluded");
        assert!(link.returns_resource(&guid(fixture::IRON)));
        assert!(
            crafting.dismantle_return(&crafting.blueprints[2]).is_none(),
            "a blacklisted item"
        );
        assert!(crafting.warnings.is_empty(), "one process is no warning");
        assert_eq!(
            crafting.global.default_blueprints,
            [guid(fixture::DISMANTLE), guid(fixture::CHOOSE_ONE)]
        );
        assert_eq!(
            crafting.global.dismantle_blacklist_resources,
            [guid(fixture::GOLD)]
        );
        assert_eq!(
            crafting.global.dismantle_blacklist_items,
            [guid(fixture::BLACKLISTED)]
        );
        Ok(())
    })
}

#[test]
fn several_dismantle_processes_link_nothing_and_warn() -> TestResult {
    let bytes = fixture::ambiguous_database().build();
    let forge = DataForge::open(&bytes)?;
    let crafting = read_crafting(&forge, None, None)?;
    assert_eq!(crafting.dismantle.len(), 2, "both processes are read");
    assert!(crafting.dismantle_process().is_none(), "none is chosen");
    assert!(
        crafting.dismantle_return(&crafting.blueprints[0]).is_none(),
        "no link instead of the first process"
    );
    assert_eq!(crafting.warnings, [CraftingWarning::AmbiguousDismantle(2)]);
    Ok(())
}

#[test]
fn modifier_ranges_of_an_unknown_type_are_counted_and_named_in_a_warning() -> TestResult {
    let bytes = fixture::unknown_ranges_database().build();
    let forge = DataForge::open(&bytes)?;
    let crafting = read_crafting(&forge, None, None)?;
    assert_eq!(
        crafting.warnings,
        [CraftingWarning::UnknownModifierRanges {
            count: 3,
            details: vec![
                "CraftingGameplayPropertyModifierValueRange_Curve".to_owned(),
                "CraftingGameplayPropertyModifierValueRange_Stepped".to_owned(),
            ],
        }]
    );
    let blueprint = crafting
        .blueprints
        .iter()
        .find(|blueprint| blueprint.guid == guid(fixture::UNKNOWN_RANGES))
        .ok_or("the blueprint with unknown ranges")?;
    assert_eq!(blueprint.unknown_range_types.len(), 3);
    Ok(())
}

#[test]
fn known_modifier_ranges_give_no_warning() -> TestResult {
    read(None, |crafting| {
        assert!(crafting.warnings.is_empty(), "{:?}", crafting.warnings);
        Ok(())
    })
}

#[test]
fn the_unreadable_count_of_a_warning_is_reported() {
    let warning = CraftingWarning::UnreadableRewardRecords(3);
    assert!(warning.to_string().contains('3'));
    assert_eq!(
        (warning.kind(), warning.count(), warning.details()),
        ("unreadable_reward_records", Some(3), &[] as &[String])
    );
    let listed = CraftingWarning::UnreadableShopInventories {
        count: 1,
        details: vec!["inv.json: the file is empty".to_owned()],
    };
    assert_eq!(listed.details(), ["inv.json: the file is empty"]);
    assert_eq!(CraftingWarning::NoShopComponents.count(), None);
}

#[test]
fn a_blueprint_without_a_tier_links_to_the_process_without_a_tier() -> TestResult {
    read(None, |crafting| {
        let bare = crafting
            .blueprints
            .iter()
            .find(|blueprint| blueprint.guid == guid(fixture::BARE))
            .ok_or("the bare blueprint")?;
        let mut unblacklisted = bare.clone();
        unblacklisted.produced_item = Some(guid(fixture::PRODUCT_A));
        let link = crafting
            .dismantle_return(&unblacklisted)
            .ok_or("a return")?;
        assert_eq!(link.tier, None, "no tier to return");
        Ok(())
    })
}

#[test]
fn a_filter_keeps_only_the_matching_blueprints_and_pools_but_all_context() -> TestResult {
    let filter = Filter::new("*/bp_craft_pick");
    read(Some(&filter), |crafting| {
        assert_eq!(crafting.blueprints.len(), 1);
        assert!(crafting.pools.is_empty() && crafting.reward_sources.is_empty());
        let link = crafting
            .dismantle_return(&crafting.blueprints[0])
            .ok_or("a dismantle return")?;
        assert_eq!(link.process.guid, guid(fixture::DISMANTLE));
        assert_eq!(crafting.global.default_blueprints.len(), 2, "the globals");
        assert_eq!(crafting.properties.len(), 1, "the properties");
        assert_eq!(
            crafting.quality_distributions.len(),
            2,
            "the quality Records"
        );
        Ok(())
    })
}

#[test]
fn a_cancelled_read_stops() -> TestResult {
    let bytes = fixture::database().build();
    let forge = DataForge::open(&bytes)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        read_crafting(&forge, None, Some(&cancel)),
        Err(CatalogError::Cancelled)
    ));
    Ok(())
}
