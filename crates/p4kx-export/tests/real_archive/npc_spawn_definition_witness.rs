// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{DataForge, StructValue, Value as Original};
use serde_json::{Value, json};

use crate::authored_definition_witness::{field, verify_array, verify_link, verify_state};
use crate::catalog_v3_consumer as consumer;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(super) fn verify_spawn(
    forge: &DataForge<'_>,
    source: Option<&Original<'_>>,
    row: &Value,
) -> TestResult {
    let Some(Original::Struct(source)) = source else {
        return verify_state(source, row);
    };
    let row = consumer::present(row)?;
    verify_spawn_links(forge, source, row)?;
    for name in ["minSpawnDelay", "maxSpawnDelay"] {
        verify_single(source.field(name), field(row, name)?)?;
    }
    for name in [
        "minGroupSize",
        "maxGroupSize",
        "maxConcurrentSpawns",
        "maxSpawns",
    ] {
        if let Some(Original::Int32(number)) = source.field(name) {
            assert_eq!(
                consumer::present(field(row, name)?)?,
                &json!(*number),
                "unmodified source count"
            );
        } else {
            verify_state(source.field(name), field(row, name)?)?;
        }
    }
    Ok(())
}

fn verify_spawn_links(forge: &DataForge<'_>, source: &StructValue<'_>, row: &Value) -> TestResult {
    verify_link(
        forge,
        "FactionIdentity",
        source.field("identity"),
        field(row, "identity")?,
    )?;
    verify_class(forge, source.field("class"), field(row, "class")?)?;
    verify_tags(forge, source.field("entityTags"), field(row, "entityTags")?)?;
    let assignment = field(row, "onSpawnAssignment")?;
    let Some(Original::Struct(original)) = source.field("onSpawnAssignment") else {
        return verify_state(source.field("onSpawnAssignment"), assignment);
    };
    let assignment = consumer::present(assignment)?;
    verify_type(original, assignment)?;
    for name in ["areaPositiveTags", "areaNegativeTags"] {
        verify_tags(forge, original.field(name), field(assignment, name)?)?;
    }
    Ok(())
}

fn verify_class(forge: &DataForge<'_>, source: Option<&Original<'_>>, row: &Value) -> TestResult {
    let Some(Original::Struct(source)) = source else {
        return verify_state(source, row);
    };
    let row = consumer::present(row)?;
    verify_type(source, row)?;
    match source.type_name() {
        "AutoSpawnTargetClass" => verify_link(
            forge,
            "EntityClassDefinition",
            source.field("npcClass"),
            field(row, "npcClass")?,
        ),
        "AutoSpawnClassPool" => verify_array(
            source.field("classPool"),
            field(row, "classPool")?,
            |slot, output| verify_link(forge, "NPCClassPool", slot, output),
        ),
        "AutoSpawnArchetype" => verify_array(
            source.field("archetype"),
            field(row, "archetype")?,
            |slot, output| verify_archetype_weight(forge, slot, output),
        ),
        _ => verify_authored(row),
    }
}

fn verify_archetype_weight(
    forge: &DataForge<'_>,
    source: Option<&Original<'_>>,
    row: &Value,
) -> TestResult {
    let Some(Original::Struct(source)) = source else {
        return verify_state(source, row);
    };
    let row = consumer::present(row)?;
    verify_single(source.field("weight"), field(row, "weight")?)?;
    verify_link(
        forge,
        "NPCArchetype",
        source.field("record"),
        field(row, "record")?,
    )
}

fn verify_tags(forge: &DataForge<'_>, source: Option<&Original<'_>>, row: &Value) -> TestResult {
    let Some(Original::Struct(source)) = source else {
        return verify_state(source, row);
    };
    verify_array(
        source.field("tags"),
        field(consumer::present(row)?, "tags")?,
        |slot, output| verify_link(forge, "Tag", slot, output),
    )
}

pub(super) fn verify_set_entry(
    forge: &DataForge<'_>,
    source: Option<&Original<'_>>,
    row: &Value,
) -> TestResult {
    let Some(Original::Struct(source)) = source else {
        return verify_state(source, row);
    };
    let row = consumer::present(row)?;
    verify_type(source, row)?;
    match source.type_name() {
        "AutoSpawnsDataRef" => verify_link(
            forge,
            "AutoSpawn",
            source.field("record"),
            field(row, "record")?,
        ),
        "AutoSpawnsDataManual" => verify_spawn(
            forge,
            source.field("autoSpawnData"),
            field(row, "autoSpawnData")?,
        ),
        _ => verify_authored(row),
    }
}

pub(super) fn verify_encounter(
    forge: &DataForge<'_>,
    source: Option<&Original<'_>>,
    row: &Value,
) -> TestResult {
    let Some(Original::Struct(source)) = source else {
        return verify_state(source, row);
    };
    let row = consumer::present(row)?;
    let preset = field(row, "encounterPreset")?;
    if let Some(Original::Enum(label)) = source.field("encounterPreset") {
        assert_eq!(
            consumer::present(preset)?.as_str(),
            Some(*label),
            "stored encounter enum"
        );
    } else {
        verify_state(source.field("encounterPreset"), preset)?;
    }
    verify_array(
        source.field("autoSpawns"),
        field(row, "autoSpawns")?,
        |slot, output| verify_link(forge, "AutoSpawns", slot, output),
    )
}

pub(crate) fn verify_single(source: Option<&Original<'_>>, row: &Value) -> TestResult {
    let Some(Original::Single(number)) = source else {
        return verify_state(source, row);
    };
    assert_eq!(
        field(consumer::present(row)?, "rawBits")?,
        &json!(format!("{:08x}", number.to_bits())),
        "each original Single has exact exported bits"
    );
    Ok(())
}

fn verify_type(source: &StructValue<'_>, row: &Value) -> TestResult {
    assert_eq!(
        consumer::present(field(row, "sourceType")?)?.as_str(),
        Some(source.type_name()),
        "the stored concrete type is retained"
    );
    Ok(())
}

fn verify_authored(row: &Value) -> TestResult {
    assert_eq!(
        field(row, "kind")?,
        "authored",
        "unrecognised variant stays distinct"
    );
    field(row, "authoredFields")?;
    Ok(())
}
