// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as archive, DcbOrigin, TestResult, at, guid};
use crate::health_origin_assertions as proof;
use crate::mission_template_graph;
use crate::template_canonical_graph::Template;
use crate::template_fixture_context;

const SOURCE: &str = "SecurityNetworkManifest";
const TEMPLATE: u128 = 0x1074_1001;
const MANIFEST: u128 = 0x1074_1002;
const DANGLING: u128 = 0x1074_1003;
const MANIFEST_INDEX: u32 = 1;
const OBJECTIVE: &str = "MissionObjectiveToken";
const HANDLER: &str = "ObjectiveHandler";
const LOCAL: &str = "ObjectiveHandler_Local";
const OVERRIDE: &str = "securityManifestToOverride";
const ROWS: &str = "/definitions/securityNetworkManifests/rows";
const MODULE_TYPE: &str = "ModuleDeclaration";
const MODULE_GUID: u128 = 0x1074_1004;
const MODULE_INDEX: u32 = 2;
const MODULE_FIELD: &str = "moduleDeclaration";
const MODULE_ROWS: &str = "/definitions/missionModuleDeclarations/rows";

#[test]
fn objective_override_joins_the_shared_manifest_by_physical_identity() -> TestResult {
    let catalog = document(None)?;
    let [manifest] = proof::rows(&catalog, ROWS)? else {
        return Err("the two authored overrides must share one physical manifest".into());
    };
    proof::present(manifest, "guid", &json!(guid(MANIFEST)), Some(true))?;
    proof::present(manifest, "recordIndex", &json!(MANIFEST_INDEX), Some(true))?;
    let graph = Template::new(&catalog, &guid(TEMPLATE))?;
    verify_objective_links(&graph)?;
    verify_module_joins(&catalog, &graph, false)?;
    assert!(
        !proof::rows(&catalog, "/exportHealth/unresolvedReferences")?
            .iter()
            .any(targets_manifest),
        "a selected compatible target produces no unresolved occurrence"
    );
    proof::closure(&catalog)
}

#[test]
fn objective_override_keeps_its_original_target_when_filtered() -> TestResult {
    let filter = Filter::new("libs/foundry/records/fixture/contract/*");
    let catalog = document(Some(&filter))?;
    assert!(
        proof::rows(&catalog, ROWS)?.is_empty(),
        "target excluded by path"
    );
    let graph = Template::new(&catalog, &guid(TEMPLATE))?;
    verify_objective_links(&graph)?;
    verify_module_joins(&catalog, &graph, true)?;
    let filtered: Vec<_> = proof::rows(&catalog, "/exportHealth/unresolvedReferences")?
        .iter()
        .filter(|link| {
            targets_manifest(link)
                && link.get("kind") == Some(&json!("contractTemplates"))
                && link.get("sourceRole") == Some(&json!("selected"))
        })
        .collect();
    assert_eq!(
        filtered.len(),
        2,
        "one health occurrence per original override"
    );
    for (index, link) in filtered.into_iter().enumerate() {
        proof::present(link, "recordGuid", &json!(guid(TEMPLATE)), Some(true))?;
        proof::present_text(
            &catalog,
            link,
            "field",
            &json!(format!(
                "objectiveTokens[{index}].objectiveHandler.{OVERRIDE}"
            )),
            Some(true),
        )?;
        assert_eq!(
            at(link, "/status")?,
            "excludedByFilter",
            "the original target was filtered"
        );
        proof::present(
            link,
            "targetRecordIndex",
            &json!(MANIFEST_INDEX),
            Some(true),
        )?;
    }
    proof::closure(&catalog)
}

fn verify_module_joins(catalog: &Value, graph: &Template<'_>, filtered: bool) -> TestResult {
    let rows = proof::rows(catalog, MODULE_ROWS)?;
    if filtered {
        assert!(rows.is_empty(), "filtered module has no published row");
    } else {
        let [module] = rows else {
            return Err("one physical module declaration expected".into());
        };
        proof::present(module, "guid", &json!(guid(MODULE_GUID)), Some(true))?;
        proof::present(module, "recordIndex", &json!(MODULE_INDEX), Some(true))?;
        proof::present(module, "module", &json!("FixtureModule"), Some(true))?;
    }
    for index in 0..4 {
        let objective = template_fixture_context::objective(graph, index)?;
        let handler = graph.field(objective, "objectiveHandler")?;
        let module = graph.field(handler, MODULE_FIELD)?;
        mission_template_graph::reference(module, &guid(MODULE_GUID), Some(MODULE_INDEX))?;
    }
    let filtered_links = proof::rows(catalog, "/exportHealth/unresolvedReferences")?
        .iter()
        .filter(|link| {
            link.pointer("/targetGuid/value") == Some(&json!(guid(MODULE_GUID)))
                && link.get("kind") == Some(&json!("contractTemplates"))
                && link.get("sourceRole") == Some(&json!("selected"))
                && link.get("status") == Some(&json!("excludedByFilter"))
        })
        .count();
    assert_eq!(
        filtered_links,
        if filtered { 4 } else { 0 },
        "each excluded objective module link has a health occurrence"
    );
    Ok(())
}

fn targets_manifest(link: &Value) -> bool {
    link.pointer("/targetGuid/value") == Some(&json!(guid(MANIFEST)))
}

fn verify_objective_links(graph: &Template<'_>) -> TestResult {
    let mut previous = None;
    for index in 0..2 {
        let node = override_at(graph, index)?;
        mission_template_graph::reference(node, &guid(MANIFEST), Some(MANIFEST_INDEX))?;
        let target = mission_template_graph::scalar(node, "recordReference")?;
        proof::present(target, "targetType", &json!(SOURCE), Some(true))?;
        let carrier = at(node, "/value")?;
        assert_eq!(
            at(carrier, "/containsEncryptedSource")?,
            true,
            "per-node source origin"
        );
        let receipts = at(carrier, "/sourceUsesRefs")?;
        assert_ne!(
            previous,
            Some(receipts),
            "distinct original occurrence receipts"
        );
        previous = Some(receipts);
    }
    mission_template_graph::reference(override_at(graph, 2)?, &guid(DANGLING), None)?;
    let null = override_at(graph, 3)?;
    assert_eq!(
        at(null, "/value/state")?,
        "null",
        "an explicit null override remains null"
    );
    Ok(())
}

fn override_at<'catalog>(graph: &Template<'catalog>, index: usize) -> TestResult<&'catalog Value> {
    let objective = template_fixture_context::objective(graph, index)?;
    let handler = graph.field(objective, "objectiveHandler")?;
    graph.field(handler, OVERRIDE)
}

fn document(filter: Option<&Filter>) -> TestResult<Value> {
    let (_folder, input) = archive::open(&database(), DcbOrigin::Encrypted)?;
    let mut options = archive::options();
    options.filter = filter;
    Ok(serde_json::from_slice(
        build_catalog(&input, &options)?.json(),
    )?)
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ContractTemplate").array("objectiveTokens", class(OBJECTIVE)))
        .structure(
            TestStruct::new(OBJECTIVE)
                .field("id", TestType::Guid)
                .field(
                    "objectiveHandler",
                    TestType::StrongPointer(HANDLER.to_owned()),
                ),
        )
        .structure(TestStruct::new(HANDLER))
        .structure(
            TestStruct::new(LOCAL)
                .parent(HANDLER)
                .field(OVERRIDE, TestType::ReferenceTo(SOURCE.to_owned()))
                .field(MODULE_FIELD, TestType::ReferenceTo(MODULE_TYPE.to_owned())),
        )
        .structure(TestStruct::new(SOURCE))
        .structure(TestStruct::new(MODULE_TYPE).field("module", TestType::String))
        .record(
            TestRecord::new(
                TEMPLATE,
                "ContractTemplate",
                "ContractTemplate.Fixture",
                "libs/foundry/records/fixture/contract/template.xml",
            )
            .value(
                "objectiveTokens",
                TestValue::Array(vec![
                    objective(0x1074_1101, TestValue::Reference(MANIFEST)),
                    objective(0x1074_1102, TestValue::Reference(MANIFEST)),
                    objective(0x1074_1103, TestValue::Reference(DANGLING)),
                    objective(0x1074_1104, TestValue::Null),
                ]),
            ),
        )
        .record(TestRecord::new(
            MANIFEST,
            SOURCE,
            "SecurityNetworkManifest.Fixture",
            "libs/foundry/records/fixture/security/manifest.xml",
        ))
        .record(
            TestRecord::new(
                MODULE_GUID,
                MODULE_TYPE,
                "ModuleDeclaration.Fixture",
                "libs/foundry/records/fixture/modules/module.xml",
            )
            .value("module", TestValue::text("FixtureModule")),
        )
}

fn objective(id: u128, target: TestValue) -> TestValue {
    let handler = TestInstance::new(LOCAL)
        .value(OVERRIDE, target)
        .value(MODULE_FIELD, TestValue::Reference(MODULE_GUID));
    TestValue::Struct(
        TestInstance::new(OBJECTIVE)
            .value("id", TestValue::Guid(id))
            .value("objectiveHandler", TestValue::Struct(handler)),
    )
}

fn class(target: &str) -> TestType {
    TestType::Class(target.to_owned())
}
