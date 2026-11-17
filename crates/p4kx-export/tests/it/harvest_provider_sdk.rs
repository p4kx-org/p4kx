// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogError;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use serde_json::{Value, json};

use super::{
    COMPOSITION, ENTITY, HARVESTABLE, PARAMS, PRESET_PLACED, RESOURCE, TestResult, build,
    build_archive, entry, guid, write_archive,
};

#[test]
fn provider_queries_match_the_canonical_source_link_and_skip_placements() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let (_, catalog) = build_archive(&archive, None)?;
    let decoded = DataForgeEntry::read(&archive)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &decoded,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let all = session.read_harvestable_provider_presets(CatalogRecordSelection::All, None)?;
    let expected = catalog["harvestableProviderPresets"]
        .as_array()
        .ok_or("provider rows in Catalog")?;
    let actual = all
        .rows
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        actual.as_slice(),
        expected,
        "every provider has the same canonical fields"
    );
    assert_eq!(
        all.source_inventory.source_count(),
        1,
        "only the DCB supplied provider evidence"
    );

    let expected = entry(&catalog, "harvestableProviderPresets", PRESET_PLACED)?;
    verify_selected_providers(&session, expected)
}

fn verify_selected_providers(session: &CatalogQuerySession<'_>, expected: &Value) -> TestResult {
    let guid = guid(PRESET_PLACED).parse()?;
    let position = expected
        .pointer("/recordIndex/value")
        .and_then(Value::as_u64)
        .ok_or("provider position")?;
    for selection in [
        CatalogRecordSelection::Guid(guid),
        CatalogRecordSelection::RecordIndex(u32::try_from(position)?),
    ] {
        let selected = session.read_harvestable_provider_presets(selection, None)?;
        let [row] = selected.rows.as_slice() else {
            return Err("one selected provider".into());
        };
        assert_eq!(
            &serde_json::to_value(row)?,
            expected,
            "selected value and origins agree"
        );
        for id in row.groups.origin().source_uses_refs() {
            assert!(
                selected
                    .source_inventory
                    .iter_uses()
                    .any(|item| item.id() == id),
                "each selected field use resolves in the selected inventory"
            );
        }
    }

    let cancel = CancelToken::default();
    cancel.cancel();
    assert!(
        matches!(
            session.read_harvestable_provider_presets(
                CatalogRecordSelection::Guid(guid),
                Some(&cancel)
            ),
            Err(CatalogError::Cancelled)
        ),
        "a cancelled request publishes no partial result"
    );
    Ok(())
}

#[test]
fn a_provider_preset_follows_each_harvestable_to_its_composition() -> TestResult {
    let (_, catalog) = build(None)?;
    let preset = entry(&catalog, "harvestableProviderPresets", PRESET_PLACED)?;
    assert_eq!(preset["className"]["value"], "HPP_Placed");
    let group = &preset["groups"]["value"][0]["value"];
    assert_eq!(group["groupName"]["value"], "SpaceShip_Mineables");
    let elements = &group["harvestables"]["value"];
    let from_preset = &elements[0]["value"];
    let from_entity = &elements[1]["value"];
    assert_eq!(
        from_preset["harvestable"]["value"]["guid"]["value"],
        guid(HARVESTABLE)
    );
    assert_eq!(from_preset["relativeProbability"]["value"]["finite"], 22.0);
    assert_eq!(
        from_preset.pointer("/geometries/value/0/value/tag/value/guid/value"),
        Some(&json!(guid(RESOURCE)))
    );
    assert_eq!(
        from_preset["spawn"]["presetClassName"]["value"],
        "Mining_Iron"
    );
    assert_eq!(
        from_preset["spawn"]["entitySelection"]["value"],
        "presetFallback"
    );
    assert_eq!(from_entity["spawn"]["entitySelection"]["value"], "override");
    assert_eq!(from_entity["harvestable"]["state"], "null");
    assert_eq!(from_entity["spawn"]["presetEntityClass"], Value::Null);
    for element in [from_preset, from_entity] {
        let entity = &element["spawn"]["entity"]["value"];
        assert_eq!(entity["guid"]["value"], guid(ENTITY));
        let mining = &entity["miningParams"]["value"];
        assert_eq!(
            mining["composition"]["value"]["guid"]["value"],
            guid(COMPOSITION)
        );
        assert_eq!(
            mining["globalParams"]["value"]["guid"]["value"],
            guid(PARAMS)
        );
        assert_eq!(mining["composition"]["sourceSetComplete"], true);
        assert!(
            mining["composition"]["sourceUsesRefs"]
                .as_array()
                .is_some_and(|refs| !refs.is_empty())
        );
    }
    verify_provider_area(preset)
}

fn verify_provider_area(preset: &Value) -> TestResult {
    let area = preset
        .pointer("/areas/value/0/value")
        .ok_or("authored provider area")?;
    assert_eq!(
        area.pointer("/name/value"),
        Some(&json!("Acidic")),
        "authored area name"
    );
    assert_eq!(
        area.pointer("/globalModifier/value/finite"),
        Some(&json!(1.0)),
        "authored global modifier"
    );
    assert_eq!(
        area.pointer("/areaType/value/objectPresetPaths/value/0/value"),
        Some(&json!(
            "Libs/ObjectPresets/PU/system/stanton/hurston/acidic_pools_01.opr"
        )),
        "authored object preset path"
    );
    verify_provider_modifier(preset, area)
}

fn verify_provider_modifier(preset: &Value, area: &Value) -> TestResult {
    let modifier = area
        .pointer("/modifiers/value/0/value")
        .ok_or("authored area modifier")?;
    assert_eq!(
        modifier.pointer("/modifier/value/finite"),
        Some(&json!(0.0)),
        "zero remains an authored value"
    );
    let target = modifier
        .pointer("/harvestableElement/value")
        .ok_or("area modifier target")?;
    assert_eq!(
        target.get("type"),
        Some(&json!("instance")),
        "target kind is an owned instance"
    );
    assert_eq!(
        target.pointer("/reference/recordGuid/value"),
        Some(&json!(guid(PRESET_PLACED))),
        "target retains the provider GUID"
    );
    let source_index = preset
        .pointer("/recordIndex/value")
        .ok_or("provider Record index")?;
    assert_eq!(
        target.pointer("/reference/targetRecordIndex/value"),
        Some(source_index),
        "target retains the physical Record position"
    );
    let slot = target
        .pointer("/reference/providerSlot/value")
        .ok_or("same-provider slot")?;
    assert_eq!(
        slot.pointer("/groupIndex/value"),
        Some(&json!(0)),
        "authored group index"
    );
    assert_eq!(
        slot.pointer("/harvestableIndex/value"),
        Some(&json!(0)),
        "authored element index"
    );
    Ok(())
}
