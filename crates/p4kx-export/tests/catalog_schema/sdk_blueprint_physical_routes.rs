// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    BlueprintSnapshot, CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
    ParsedCatalogQueries, SourceValueRef,
};
use p4kx_extraction::{Archive, Guid};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use super::super::{
    BLUEPRINT_DEFINITIONS, BLUEPRINT_PRESENTATION, BLUEPRINT_TIERS, CRAFTING_GLOBAL, Route,
    TestResult,
};

const PRIMARY: u32 = 0;
const ALIAS: u32 = 1;
const GLOBAL: u32 = 2;
const OWNER_GUID: u128 = 1;
const OWNER_TYPE: &str = "CraftingBlueprintRecord";
const BODY_TYPE: &str = "CraftingBlueprintBase";
const TIER_TYPE: &str = "CraftingBlueprintTier";
const GLOBAL_TYPE: &str = "CraftingGlobalParams";
const DEFAULTS_TYPE: &str = "DefaultBlueprintSelection_Whitelist";

pub(super) fn read() -> TestResult<[Route; 4]> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("sdk_physical_blueprints.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database().build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let ([definitions, tiers, presentations], guid) = blueprint_routes(&parsed)?;
    Ok([
        definitions,
        tiers,
        presentations,
        global_route(&parsed, guid)?,
    ])
}

fn blueprint_routes(parsed: &ParsedCatalogQueries<'_>) -> TestResult<([Route; 3], Guid)> {
    let selection = CatalogRecordSelection::RecordIndex(ALIAS);
    let (routes, guid) =
        parsed.with_blueprints(selection, None, |snapshot| selected_routes(&snapshot))??;
    for selection in [
        CatalogRecordSelection::Guid(guid),
        CatalogRecordSelection::All,
    ] {
        parsed.with_blueprints(selection, None, |snapshot| -> TestResult {
            let [first, second] = snapshot.definitions() else {
                return Err("both physical Blueprint owners must survive".into());
            };
            if first.header().record_index().value() != Some(&PRIMARY)
                || second.header().record_index().value() != Some(&ALIAS)
            {
                return Err("the Blueprint aliases must keep their physical order".into());
            }
            Ok(())
        })??;
    }
    Ok((routes, guid))
}

fn selected_routes(snapshot: &BlueprintSnapshot<'_>) -> TestResult<([Route; 3], Guid)> {
    let [definition] = snapshot.definitions() else {
        return Err("one Blueprint definition for the selected physical alias".into());
    };
    let [first, second] = snapshot.tiers() else {
        return Err("both original tier occurrences of the selected Blueprint".into());
    };
    let [presentation] = snapshot.presentations() else {
        return Err("one Blueprint presentation for the selected physical alias".into());
    };
    if definition.header().record_index().value() != Some(&ALIAS)
        || first.tier_index().value() != Some(&0)
        || second.tier_index().value() != Some(&1)
        || presentation.first_tier().map(SourceValueRef::value_index)
            != Some(first.source().value_index())
        || presentation.default_available().value() != Some(&true)
    {
        return Err("selected Blueprint tiers and authored presentation must keep their positions and default".into());
    }
    for owner in [
        first.blueprint(),
        second.blueprint(),
        presentation.blueprint(),
    ] {
        if owner.record_index() != ALIAS || owner.input_ref() != definition.input_ref() {
            return Err(
                "every interpreted Blueprint row must point to the same physical definition".into(),
            );
        }
    }
    let guid = definition
        .header()
        .guid()
        .value()
        .copied()
        .ok_or("the original Blueprint GUID")?;
    Ok((
        [
            Route::queried_rows(
                BLUEPRINT_DEFINITIONS,
                snapshot.definitions(),
                "typed owner and synthetic producer parity witnessed; complete installed producer parity open",
            )?,
            Route::queried_rows(
                BLUEPRINT_TIERS,
                snapshot.tiers(),
                "original tier positions and physical links exercised; complete installed producer parity open",
            )?,
            Route::queried_rows(
                BLUEPRINT_PRESENTATION,
                snapshot.presentations(),
                "physical links and a default selection exercised; complete installed producer and dependency parity open",
            )?,
        ],
        guid,
    ))
}

fn global_route(parsed: &ParsedCatalogQueries<'_>, guid: Guid) -> TestResult<Route> {
    parsed.with_crafting_global_params(
        CatalogRecordSelection::RecordIndex(GLOBAL),
        None,
        |snapshot| {
            let [row] = snapshot.rows() else {
                return Err("one selected Crafting Global definition".into());
            };
            let default = row
                .default_blueprint_selection()
                .value()
                .ok_or("authored Crafting Global default selection")?;
            let [slot] = default
                .blueprint_records()
                .value()
                .map(Vec::as_slice)
                .ok_or("authored default Blueprint slots")?
            else {
                return Err("one original default Blueprint slot".into());
            };
            if row.source().record_index().value() != Some(&GLOBAL)
                || slot.value().and_then(|link| link.guid().value().copied()) != Some(guid)
            {
                return Err(
                    "the selected global must retain its own position and authored Blueprint GUID"
                        .into(),
                );
            }
            Route::queried_rows(CRAFTING_GLOBAL, snapshot.rows(), None)
        },
    )?
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(OWNER_TYPE)
                .field("blueprint", TestType::StrongPointer(BODY_TYPE.to_owned())),
        )
        .structure(TestStruct::new(BODY_TYPE).array("tiers", TestType::Class(TIER_TYPE.to_owned())))
        .structure(TestStruct::new(TIER_TYPE).field("fixtureOrdinal", TestType::UInt32))
        .structure(TestStruct::new(GLOBAL_TYPE).field(
            "defaultBlueprintSelection",
            TestType::StrongPointer(DEFAULTS_TYPE.to_owned()),
        ))
        .structure(TestStruct::new(DEFAULTS_TYPE).array("blueprintRecords", TestType::Reference))
        .record(blueprint("Primary"))
        .record(blueprint("Alias"))
        .record(
            TestRecord::new(
                2,
                GLOBAL_TYPE,
                "CraftingGlobalParams.Fixture",
                "libs/foundry/records/crafting/globalparams/fixture.xml",
            )
            .value(
                "defaultBlueprintSelection",
                TestValue::Struct(TestInstance::new(DEFAULTS_TYPE).value(
                    "blueprintRecords",
                    TestValue::Array(vec![TestValue::Reference(OWNER_GUID)]),
                )),
            ),
        )
}

fn blueprint(name: &str) -> TestRecord {
    let tiers = (0..2)
        .map(|ordinal| {
            TestValue::Struct(
                TestInstance::new(TIER_TYPE).value("fixtureOrdinal", TestValue::UInt32(ordinal)),
            )
        })
        .collect();
    TestRecord::new(
        OWNER_GUID,
        OWNER_TYPE,
        format!("{OWNER_TYPE}.BP_CRAFT_Fixture{name}"),
        format!("libs/foundry/records/crafting/blueprints/crafting/fixture/{name}.xml"),
    )
    .value(
        "blueprint",
        TestValue::Struct(TestInstance::new(BODY_TYPE).value("tiers", TestValue::Array(tiers))),
    )
}
