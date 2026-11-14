// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;

use p4kx_extraction::{Archive, ArchiveWatch, DataForge, DataForgeEntry, DataType};
use p4kx_testkit::{real_archive_key, real_archive_path};

use crate::real_archive_verification::print_line;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SINGLE_LAYER: &str = "LootV3SecondaryChoicesSingleLayerRecord";
const MULTI_LAYER: &str = "LootV3SecondaryChoicesMultiLayerRecord";
const THEME: &str = "LootThemeRecord";
const CONTEXT_PRESET: &str = "HierarchicalContextualDataPresetRecord";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn loot_source_declarations_resolve_to_their_physical_schema() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let build = archive
        .build()
        .ok_or("the configured Archive has no Build")?;
    if [
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    ]
    .into_iter()
    .any(str::is_empty)
    {
        return Err("the configured Archive has incomplete Build identity".into());
    }
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let declared = forge
        .structs()
        .map(|row| row.name())
        .collect::<Result<BTreeSet<_>, _>>()?;
    check_secondary_choices(&forge, &declared)?;
    check_themes(&forge, &declared)?;
    check_context(&forge, &declared)?;
    check_entity(&forge, &declared)?;
    let mut physical = BTreeMap::<_, usize>::new();
    for record in forge.records() {
        watch.poll()?;
        let name = record.type_name()?;
        if [SINGLE_LAYER, MULTI_LAYER, THEME, CONTEXT_PRESET].contains(&name) {
            *physical.entry(name).or_default() += 1;
        }
    }
    assert!(
        physical.keys().all(|name| declared.contains(name)),
        "every physical source family has a schema declaration"
    );
    watch.check()?;
    print_line(&format!(
        "Loot source shape verified. Physical source families {physical:?}"
    ));
    Ok(())
}

fn check_secondary_choices(forge: &DataForge<'_>, declared: &BTreeSet<&str>) -> TestResult {
    if declared.contains(SINGLE_LAYER) {
        field(
            forge,
            SINGLE_LAYER,
            "choices",
            DataType::Class,
            true,
            "LootV3SecondaryChoiceEntry",
        )?;
        field(
            forge,
            "LootV3SecondaryChoiceEntry",
            "selector",
            DataType::StrongPointer,
            false,
            "LootV3SecondaryChoiceEntrySelector_Base",
        )?;
        field(
            forge,
            "LootConstraints",
            "secondaryChoices",
            DataType::StrongPointer,
            false,
            "LootV3SecondaryChoicesRecordRef_Base",
        )?;
    }
    if declared.contains("LootV3SecondaryChoicesRecordRef_SingleLayer") {
        field(
            forge,
            "LootV3SecondaryChoicesRecordRef_SingleLayer",
            "singleLayerRecord",
            DataType::Reference,
            false,
            SINGLE_LAYER,
        )?;
    }
    if declared.contains(MULTI_LAYER) {
        field(
            forge,
            MULTI_LAYER,
            "layers",
            DataType::Reference,
            true,
            SINGLE_LAYER,
        )?;
    }
    Ok(())
}

fn check_entity(forge: &DataForge<'_>, declared: &BTreeSet<&str>) -> TestResult {
    if declared.contains("LootGenerationComponentParams") {
        field(
            forge,
            "LootGenerationComponentParams",
            "lootConfig",
            DataType::Class,
            false,
            "LootConfig",
        )?;
        field(
            forge,
            "LootConfig",
            "lootConstraints",
            DataType::Class,
            false,
            "LootConstraints",
        )?;
    }
    Ok(())
}
fn check_themes(forge: &DataForge<'_>, declared: &BTreeSet<&str>) -> TestResult {
    for (owner, name, target) in [
        (
            "SubHarvestableConfigRecord",
            "subConfig",
            "SubHarvestableConfig",
        ),
        (
            "SubHarvestableMultiConfigRecord",
            "multiConfig",
            "SubHarvestableMultiConfig",
        ),
    ] {
        if declared.contains(owner) {
            field(forge, owner, name, DataType::Class, false, target)?;
        }
    }
    if declared.contains(THEME) {
        field(
            forge,
            THEME,
            "theme",
            DataType::StrongPointer,
            false,
            "LootTheme_NoRef",
        )?;
        for owner in ["SubHarvestableConfig", "SubHarvestableMultiConfig"] {
            field(forge, owner, "lootThemes", DataType::Reference, true, THEME)?;
        }
        field(
            forge,
            "LootConstraints",
            "themes",
            DataType::Reference,
            true,
            THEME,
        )?;
        field(
            forge,
            "LootGenerationGlobalParams",
            "themeCategoryOrdering",
            DataType::Class,
            false,
            "TagList",
        )?;
    }
    Ok(())
}

fn check_context(forge: &DataForge<'_>, declared: &BTreeSet<&str>) -> TestResult {
    if declared.contains(CONTEXT_PRESET) {
        field(
            forge,
            CONTEXT_PRESET,
            "HCDs",
            DataType::StrongPointer,
            true,
            "HierarchicalContextualData",
        )?;
        field(
            forge,
            "LocationLootGenerationContextData",
            "themes",
            DataType::Reference,
            true,
            THEME,
        )?;
        field(
            forge,
            "LocationLootGenerationContextData",
            "parentThemesToIgnore",
            DataType::StrongPointer,
            false,
            "LootThemeSelector_Base",
        )?;
    }
    Ok(())
}

fn field(
    forge: &DataForge<'_>,
    owner: &str,
    name: &str,
    data_type: DataType,
    array: bool,
    target: &str,
) -> TestResult {
    let definition = forge
        .structs()
        .find(|row| row.name() == Ok(owner))
        .ok_or_else(|| format!("missing source type {owner}"))?;
    let property = definition
        .properties()
        .find(|property| property.name() == Ok(name))
        .ok_or_else(|| format!("missing source declaration {owner}.{name}"))?;
    assert_eq!(property.data_type()?, data_type, "{owner}.{name} type");
    assert_eq!(property.is_array(), array, "{owner}.{name} cardinality");
    let original = property
        .target_struct()
        .ok_or("missing declared target struct")?;
    assert_eq!(original.name()?, target, "{owner}.{name} target type");
    Ok(())
}
