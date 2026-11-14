// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::SocContainerCapture;
use p4kx_blender::{SocScene, soc_scene_schema};
use p4kx_extraction::{Archive, CancelToken};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, SOC_IDENTITY, TestChunkFile, TestContainer, TestEntity,
    TestEntry, TestInstance, TestObjectContainer, TestSoc, TestStruct, TestType, TestValue,
    entity_schema, test_key,
};
use serde_json::{Value, json};

type Check<T = ()> = Result<T, Box<dyn std::error::Error>>;

const CONTAINER: &str = "fixture.socpak";
const ONE: &str = "objects/one/door.cga";
const AMBIGUOUS: &str = "objects/shared/door.cga";
const LOCAL: &str = "objects/local/door.cga";
const NO_CONTROLLER: &str = "objects/other/door.cga";
const NON_CGA: &str = "objects/one/door.cgf";
const MISSING: &str = "objects/missing/door.cga";
const AUTHORED_ONE: &str = "Objects\\ONE\\DOOR.CGA";
const GUID: u128 = 0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa;
const GUID_TEXT: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
const GEOMETRY: &str = "/occurrences/0/streams/0/geometry";

#[test]
fn geometry_controller_candidates_require_the_exact_physical_root_entry() -> Check {
    let scene = fixture(true)?;
    let schema = soc_scene_schema();
    jsonschema::validator_for(&schema)?
        .validate(&scene)
        .map_err(|error| error.to_string())?;
    let refs = rows(&scene, GEOMETRY)?;
    let [one, ambiguous, local, absent, cgf, missing] = refs else {
        return Err("all six positioned geometry slots".into());
    };
    verify_one(&scene, one)?;
    verify_ambiguous(ambiguous)?;
    verify_disposition(local, "outsideRootDataforge", true)?;
    verify_disposition(absent, "noControllerObserved", true)?;
    verify_disposition(cgf, "nonCga", true)?;
    verify_disposition(missing, "geometryUnresolved", false)?;
    verify_placements_and_class(&scene)
}

#[test]
fn unavailable_controller_inventory_does_not_claim_that_no_controller_exists() -> Check {
    let scene = fixture(false)?;
    let root = rows(&scene, GEOMETRY)?.first().ok_or("unique root CGA")?;
    verify_disposition(root, "controllerSourceUnavailable", false)
}

fn verify_one(scene: &Value, row: &Value) -> Check {
    verify_disposition(row, "candidate", true)?;
    let candidate = at(row, "/controllerCandidates/value/candidate")?;
    let expected = [
        ("/value/geometryPath/value", json!(AUTHORED_ONE)),
        ("/value/recordGuid/value", json!(GUID_TEXT)),
        ("/value/controllerPath/value", json!("controller-one.xml")),
        ("/value/databasePath/value", json!("database-one.adb")),
        ("/value/subGeometry/value", json!([])),
    ];
    for (path, value) in expected {
        assert_eq!(at(candidate, path)?, &value, "exported original {path}");
    }
    assert_eq!(
        at(candidate, "/value/geometrySource/value")?,
        at(row, "/target/value/source")?,
        "both original selectors reach the same physical archive entry"
    );
    for use_ in ["/value/geometryPath", "/value/controllerPath"] {
        assert_eq!(
            at(at(candidate, use_)?, "/containsEncryptedSource")?,
            &json!(true),
            "independent authored DataForge field uses the encrypted entry"
        );
    }
    let refs = rows(candidate, "/value/geometrySource/sourceUsesRefs")?;
    let inventory = rows(scene, "/sourceInventory/uses")?;
    assert!(
        inventory.iter().any(|use_| {
            use_.get("id").is_some_and(|id| refs.contains(id))
                && use_.get("scope").and_then(Value::as_str) == Some("archiveDirectoryMetadata")
                && use_.pointer("/location/field").and_then(Value::as_str)
                    == Some("archiveEntryPath")
        }),
        "physical candidate has an exact directory use"
    );
    Ok(())
}

fn verify_ambiguous(row: &Value) -> Check {
    verify_disposition(row, "ambiguous", true)?;
    let [first, second] = rows(row, "/controllerCandidates/value/candidates")? else {
        return Err("all same-entry declaration occurrences".into());
    };
    assert_ne!(
        at(first, "/value/recordIndex/value")?,
        at(second, "/value/recordIndex/value")?,
        "distinct physical records remain separate"
    );
    assert_eq!(
        at(first, "/value/subGeometry/value")?,
        &json!([]),
        "root declaration remains positioned"
    );
    assert_eq!(
        at(second, "/value/subGeometry/value")?,
        &json!([0]),
        "nested declaration retains its ordinal"
    );
    Ok(())
}

fn verify_disposition(row: &Value, state: &str, complete: bool) -> Check {
    let value = at(row, "/controllerCandidates")?;
    assert_eq!(
        at(value, "/value/type")?,
        &json!(state),
        "observed selection status"
    );
    assert_eq!(
        at(value, "/sourceSetComplete")?,
        &json!(complete),
        "candidate inventory completeness"
    );
    Ok(())
}

fn verify_placements_and_class(scene: &Value) -> Check {
    let placements = rows(scene, "/includedObjects/0/tables/value/rows")?;
    let [first, second] = placements else {
        return Err("two original placements".into());
    };
    for placement in [first, second] {
        assert_eq!(
            at(placement, "/data/value/modelSlot")?,
            &json!(0),
            "repeated placements retain independent roles with one geometry slot"
        );
    }
    assert_eq!(
        at(
            scene,
            "/occurrences/0/streams/0/member/data/0/entities/0/class/value/type"
        )?,
        &json!("matched"),
        "XML class GUID remains an authoritative independent relationship"
    );
    Ok(())
}

fn at<'a>(value: &'a Value, path: &str) -> Check<&'a Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("missing output {path}").into())
}

fn rows<'a>(value: &'a Value, path: &str) -> Check<&'a [Value]> {
    at(value, path)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("expected array {path}").into())
}

fn fixture(include_dataforge: bool) -> Check<Value> {
    let folder = tempfile::tempdir()?;
    let archive_path = folder.path().join("source.p4k");
    let mut archive = ArchiveBuilder::new()
        .entry(TestEntry::stored(CONTAINER, container()))
        .entry(TestEntry::stored(format!("Data/{ONE}"), b"unused synthetic model").encrypted())
        .entry(TestEntry::stored(
            format!("Data/{AMBIGUOUS}"),
            b"unused synthetic model",
        ))
        .entry(TestEntry::stored(
            format!("Data/{LOCAL}"),
            b"root shadowed by nested SOC container",
        ))
        .entry(TestEntry::stored(
            format!("Data/{NO_CONTROLLER}"),
            b"unused synthetic model",
        ))
        .entry(TestEntry::stored(
            format!("Data/{NON_CGA}"),
            b"unused synthetic mesh",
        ));
    if include_dataforge {
        archive = archive.entry(TestEntry::stored("Data/Game2.dcb", forge()).encrypted());
    }
    archive.write_to(&archive_path)?;
    let archive = Archive::open_with_key(archive_path, Some(test_key()))?;
    let entry = archive.entries().first().ok_or("physical SOC container")?;
    let container = archive.open_container(entry)?;
    let cancel = CancelToken::new();
    let capture = SocContainerCapture::read(&[&archive, &container], &cancel)?;
    Ok(serde_json::to_value(SocScene::from_archive_capture(
        &capture, &archive, &cancel,
    )?)?)
}

fn container() -> Vec<u8> {
    let descriptor = TestObjectContainer::new("fixture").stream("fixture.soc");
    let xml = format!(
        r#"<Entities><Entity EntityClassGUID="{GUID_TEXT}">
          <PropertiesDataCore><SCItemDoor/></PropertiesDataCore>
        </Entity></Entities>"#
    );
    let soc = [ONE, AMBIGUOUS, LOCAL, NO_CONTROLLER, NON_CGA, MISSING]
        .into_iter()
        .fold(TestSoc::new(), TestSoc::model)
        .object(0, SOC_IDENTITY)
        .object(0, SOC_IDENTITY);
    let bytes = TestChunkFile::crch()
        .chunk(0x0002, 5, vec![0; 4])
        .chunk(0x0010, 1, soc.chunk())
        .chunk(0x0004, 3, xml.into_bytes())
        .bytes();
    TestContainer::new()
        .entry(TestEntry::stored("fixture.xml", descriptor.xml()))
        .entry(TestEntry::stored("fixture.soc", bytes))
        .entry(TestEntry::stored(
            format!("Data/{LOCAL}"),
            b"physical nested target",
        ))
        .build()
}

fn forge() -> Vec<u8> {
    let one = class(GUID, "one", AUTHORED_ONE)
        .component(controller("one"))
        .component(TestValue::Struct(TestInstance::new("SCItemDoorParams")));
    let ambiguous = class(2, "ambiguous", AMBIGUOUS).component(controller("A"));
    let nested = class(3, "nested", LOCAL)
        .geometry_variant("foo", AMBIGUOUS, None)
        .component(controller("B"));
    let no_controller = class(4, "no_controller", NO_CONTROLLER);
    entity_schema(DataForgeBuilder::new())
        .structure(
            TestStruct::new("SAnimationControllerParams")
                .parent("DataForgeComponentParams")
                .field("AnimationController", TestType::String)
                .field("AnimationDatabase", TestType::String),
        )
        .structure(
            TestStruct::new("SCItemDoorParams")
                .parent("DataForgeComponentParams")
                .field("FragName", TestType::String),
        )
        .record(one.record())
        .record(ambiguous.record())
        .record(nested.record())
        .record(no_controller.record())
        .build()
}

fn class(guid: u128, name: &str, model: &str) -> TestEntity {
    TestEntity::new(
        guid,
        name,
        format!("libs/foundry/records/entities/test/{name}"),
    )
    .geometry(model, None)
}

fn controller(name: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SAnimationControllerParams")
            .value(
                "AnimationController",
                TestValue::text(format!("controller-{name}.xml")),
            )
            .value(
                "AnimationDatabase",
                TestValue::text(format!("database-{name}.adb")),
            ),
    )
}
