// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ArchiveFiles, LoadoutError, LoadoutFileError, LoadoutFiles, VehicleImplementation,
};
use p4kx_extraction::{Field, StructValue, Value as Raw};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry};
use serde_json::{Value, json};

use super::{BASE, PRIMARY, diagnostics};
use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

const FIRST: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Modifications/First.xml";
const SECOND: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Modifications/Second.xml";
const XML: &[u8] = br#"<Vehicle><Modifications><Modification name="variant" patchFile="Modifications/First.xml"/><Modification NAME="Variant" patchFile="Modifications/Second.xml"/></Modifications></Vehicle>"#;

#[test]
fn an_exact_modification_keeps_only_its_original_xml_name_selector() -> TestResult {
    check_choice("Variant", 4, "NAME", SECOND, 3)
}

#[test]
fn an_ascii_case_fallback_keeps_the_first_original_xml_name_selector() -> TestResult {
    check_choice("VARIANT", 3, "name", FIRST, 4)
}

fn check_choice(request: &str, winner: u64, name: &str, selected: &str, loser: u64) -> TestResult {
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8))
        .record(
            TestEntity::new(
                0x5742,
                "Mixed",
                "libs/foundry/records/entities/spaceships/mixed.xml",
            )
            .attach_type("NOITEM_Vehicle")
            .vehicle(PRIMARY, request)
            .record(),
        )
        .build();
    let source = Fixture::open(
        &bytes,
        Input::Plain,
        vec![TestEntry::stored(format!("Data/{BASE}"), XML).encrypted()],
    )?;
    let original_ordinal = original(&source, request, selected)?;
    let (_, catalog) = source.diagnostic()?;
    let example = selected_example(&catalog)?;
    let field = check::present_text(&catalog, example, "field", &json!(selected), Some(true))?;
    let uses = check::uses(&catalog, field)?;
    let kind = check::present_text(
        &catalog,
        example,
        "sourceType",
        &json!("EntityClassDefinition"),
        Some(false),
    )?;
    check::header(&catalog, kind, 0, "sourceType", "selector", "archiveEntry")?;
    assert_eq!(
        example.pointer("/targetGuid/state"),
        Some(&json!("absent")),
        "the file is not a target GUID"
    );
    assert!(
        uses.iter()
            .any(|used| used.get("role") == Some(&json!("selector"))
                && used.pointer("/location/namespace") == Some(&json!("dataforgeValue"))
                && used.pointer("/location/indices")
                    == Some(&json!(["0", &original_ordinal.to_string()]))
                && used.pointer("/location/field") == Some(&json!("$value"))),
        "the actual original authored DataForge Modification selection survives"
    );
    for role in ["payload", "conversion"] {
        assert!(
            uses.iter()
                .any(|used| xml_use(used, winner, "@patchFile", role)),
            "the winning XML patchFile is its actual {role}"
        );
    }
    check::closure(&catalog)?;
    name_selection(&catalog, &uses, winner, name, loser);
    Ok(())
}

fn selected_example(catalog: &Value) -> TestResult<&Value> {
    assert_eq!(
        check::rows(catalog, "/ships")?.len(),
        1,
        "the genuine original vehicle row remains"
    );
    let rows = diagnostics(catalog)?;
    assert_eq!(
        rows.len(),
        1,
        "the actual selected patch is the only missing Vehicle diagnostic"
    );
    let row = rows.first().ok_or("the selected missing patch")?;
    assert_eq!(
        row.get("count"),
        Some(&json!(1)),
        "one original missing patch"
    );
    let examples = check::rows(row, "/examples")?;
    assert_eq!(examples.len(), 1, "one actual selected example");
    let example = examples.first().ok_or("the original patch example")?;
    Ok(example)
}

fn name_selection(catalog: &Value, uses: &[&Value], winner: u64, name: &str, loser: u64) {
    assert!(
        uses.iter().any(
            |used| xml_use(used, winner, &format!("@{name}"), "selector")
                && check::source(catalog, used)
                    .is_ok_and(|source| source.pointer("/path/value")
                        == Some(&json!(format!("Data/{BASE}"))))
        ),
        "the actual winning XML name supplies its own Selector from the original vehicle document"
    );
    assert!(
        !uses.iter().any(|used| ["@name", "@NAME"]
            .iter()
            .any(|name| xml_use(used, loser, name, "selector"))),
        "the losing independent XML candidate is not a Selector"
    );
    assert!(
        !uses
            .iter()
            .any(|used| ["payload", "conversion"].iter().any(|role| xml_use(
                used,
                winner,
                &format!("@{name}"),
                role
            ))),
        "choosing a Modification name neither supplies nor converts its patch file text"
    );
}

fn xml_use(used: &Value, node: u64, field: &str, role: &str) -> bool {
    used.get("role") == Some(&json!(role))
        && used.pointer("/location/namespace") == Some(&json!("xml"))
        && used.pointer("/location/indices") == Some(&json!([node.to_string()]))
        && used.pointer("/location/field") == Some(&json!(field))
}

fn original(source: &Fixture, request: &str, selected: &str) -> TestResult<u64> {
    let forge = source.entry.open()?;
    let record = forge
        .record(0)
        .ok_or("the actual original Vehicle Record")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record) && tree.error_count() == 0,
        "the original record/tree is genuine and readable"
    );
    let Some(Raw::Array(components)) = tree.root().field("Components") else {
        return Err("the genuine original Components array".into());
    };
    let component = components
        .iter()
        .find_map(|value| match value {
            Raw::Struct(value) if value.type_name() == "VehicleComponentParams" => Some(value),
            _ => None,
        })
        .ok_or("the actual original vehicle component")?;
    let value = component
        .field("modification")
        .ok_or("the actual original Modification request")?;
    assert!(
        matches!(value, Raw::String(actual) if *actual == request),
        "the unconverted original request text"
    );
    let ordinal =
        original_value_ordinal(tree.root(), value).ok_or("the genuine Modification ordinal")?;
    original_xml(source, request, selected)?;
    Ok(ordinal)
}

fn original_xml(source: &Fixture, request: &str, selected: &str) -> TestResult {
    let files = ArchiveFiles::new(&source.archive);
    let bytes = files.read(BASE)?;
    assert_eq!(bytes, XML, "actual decrypted base XML bytes");
    let document = roxmltree::Document::parse(std::str::from_utf8(&bytes)?)?;
    let authored: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("Modification"))
        .map(|node| {
            (
                node.id().get(),
                node.attribute("name").or_else(|| node.attribute("NAME")),
                node.attribute("patchFile"),
            )
        })
        .collect();
    assert_eq!(
        authored,
        [
            (3, Some("variant"), Some("Modifications/First.xml")),
            (4, Some("Variant"), Some("Modifications/Second.xml"))
        ],
        "the actual separate original candidate slots"
    );
    for path in [PRIMARY, FIRST, SECOND] {
        assert!(
            matches!(files.read(path), Err(LoadoutFileError::NotFound)),
            "the actual authored or fallback candidate is absent"
        );
    }
    let vehicle = VehicleImplementation::read(&files, "Mixed", PRIMARY, Some(request))?;
    assert_eq!(
        vehicle.path(),
        BASE,
        "the real class fallback selects the actual original base"
    );
    let [LoadoutError::MissingVehicleImplementation { path }] = vehicle.issues() else {
        return Err("one real missing explicit patch".into());
    };
    assert_eq!(
        path.value(),
        selected,
        "actual production selection prefers exact then first ASCII-CI"
    );
    Ok(())
}

fn original_value_ordinal(root: &StructValue<'_>, target: &Raw<'_>) -> Option<u64> {
    let mut stack: Vec<_> = root.fields().iter().rev().map(Field::value).collect();
    let mut ordinal = 0_u64;
    while let Some(value) = stack.pop() {
        if std::ptr::eq(value, target) {
            return Some(ordinal);
        }
        match value {
            Raw::Struct(nested) => stack.extend(nested.fields().iter().rev().map(Field::value)),
            Raw::Array(items) => stack.extend(items.iter().rev()),
            _ => {}
        }
        ordinal += 1;
    }
    None
}
