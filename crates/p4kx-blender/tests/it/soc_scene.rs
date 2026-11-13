// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_assembly::{ArchiveModels, Interior, InteriorOptions, SocContainerCapture};
use p4kx_blender::{
    InteriorScene, PackageError, PackageInfo, SocScene, manifest_schema, soc_scene_schema,
    write_interior_package,
};
use p4kx_extraction::{Archive, CancelToken, GlbOptions};
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestContainer, TestEntry};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const GUID: &str = "11111111-1111-1111-1111-111111111111";
const CLASS: &str = "22222222-2222-2222-2222-222222222222";
const MANUAL: &str = "SItemPortLoadoutManualParams";

fn fixture() -> TestResult<(tempfile::TempDir, Archive, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("soc-scene.p4k");
    let xml = format!(
        r#"<Entities><Entity EntityCryGUID="{GUID}" EntityClassGUID="{CLASS}" Pos="2,3,4">
        <AreaComponent><Entities><Entity EntityCryGUID="{GUID}"/></Entities>
          <Segment Type="Box" BoxMin="0,0,0" BoxMax="1,2,3" transform_ls="Q:1,0,0,0|T:4,5,6"/>
        </AreaComponent>
        <PropertiesDataCore><SCItemDoor __type="SCItemDoorParams"/>
          <EntityComponentDefaultLoadout><loadout __type="{MANUAL}">
            <entries><SItemPortLoadoutEntryParams entityClassReference="{CLASS}">
              <loadout __type="{MANUAL}"><entries/></loadout>
            </SItemPortLoadoutEntryParams></entries>
          </loadout></EntityComponentDefaultLoadout>
        </PropertiesDataCore>
        <EntityLinks><Link Name="test loop" TargetCryGUID="{GUID}"/><Link/></EntityLinks>
        </Entity></Entities>"#
    );
    let descriptor = br#"<ObjectContainer><StreamingOCs><OC name="placed.soc"/><OC name="placed.soc"/></StreamingOCs></ObjectContainer>"#;
    let registry =
        format!(r#"<EntityComponentRoom><Entity CryGUID="{GUID}"/></EntityComponentRoom>"#);
    let soc = TestChunkFile::crch()
        .chunk(0x0004, 3, xml.into_bytes())
        .bytes();
    let container = TestContainer::new()
        .entry(TestEntry::stored("selected.xml", descriptor))
        .entry(TestEntry::stored(
            "metadata/entitycomponentroom.xml",
            registry,
        ))
        .entry(TestEntry::stored("placed.soc", soc));
    ArchiveBuilder::new()
        .entry(TestEntry::stored("selected.socpak", container.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = archive
        .entries()
        .first()
        .ok_or("original container Entry")?;
    let container = archive.open_container(entry)?;
    Ok((folder, archive, container))
}

fn scene() -> TestResult<Value> {
    let (_folder, archive, container) = fixture()?;
    let capture = SocContainerCapture::read(&[&archive, &container], &CancelToken::new())?;
    Ok(serde_json::to_value(SocScene::from_capture(&capture)?)?)
}

#[test]
fn soc_scene_projects_scoped_room_and_door_links_with_per_value_uses() -> TestResult {
    let value = scene()?;
    let schema = soc_scene_schema();
    let validator = jsonschema::validator_for(&schema)?;
    validator
        .validate(&value)
        .map_err(|error| error.to_string())?;
    assert_eq!(
        value["schemaVersion"], 1,
        "independent versioned wire schema"
    );
    let entity = value
        .pointer("/occurrences/0/streams/0/member/data/0/entities/0")
        .ok_or("original entity wire position")?;
    assert_eq!(entity["guid"]["value"], GUID, "the original entity GUID");
    assert_eq!(
        entity["classGuid"]["value"], CLASS,
        "the original class GUID"
    );
    assert_eq!(
        entity["links"][0]["target"]["value"]["type"], "matched",
        "the self-link is retained once without recursion"
    );
    assert_eq!(
        entity["links"][1]["target"]["state"], "absent",
        "omitted link target"
    );
    let room = &entity["areas"][0]["links"][0]["target"];
    assert_eq!(room["value"]["ordinal"], 0, "physical room declaration");
    let fields = [&entity["links"][0]["target"], room];
    let uses = value["sourceInventory"]["uses"]
        .as_array()
        .ok_or("source uses inventory")?;
    for field in fields {
        assert_eq!(
            field["containsEncryptedSource"], true,
            "encrypted outer container marks each contributing datum"
        );
        let refs = field["sourceUsesRefs"]
            .as_array()
            .ok_or("per-datum source uses")?;
        let roles: Vec<_> = uses
            .iter()
            .filter(|used| refs.contains(&used["id"]))
            .filter_map(|used| used["role"].as_str())
            .collect();
        assert!(
            roles.contains(&"selector") && roles.contains(&"payload"),
            "exact selector plus target payload"
        );
    }
    verify_raw_document(&value)?;
    verify_values(entity)
}

fn verify_raw_document(value: &Value) -> TestResult {
    let first = value
        .pointer("/occurrences/0/streams/0/member/data/0/xml")
        .and_then(Value::as_u64);
    let reused = value
        .pointer("/occurrences/0/streams/1/member/data/0/xml")
        .and_then(Value::as_u64);
    assert!(
        first.is_some() && first == reused,
        "repeated selectors refer to one physical XML document"
    );
    let index = usize::try_from(first.ok_or("original raw document reference")?)?;
    let document = value
        .get("documents")
        .and_then(Value::as_array)
        .and_then(|docs| docs.get(index))
        .ok_or("original raw document")?;
    assert_eq!(
        document.get("chunk").and_then(Value::as_u64),
        Some(0),
        "raw coordinate owns the exact chunk"
    );
    assert_eq!(
        document.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "common raw document origin is complete"
    );
    assert_eq!(
        document
            .pointer("/nodes/1/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "raw node is directly filterable"
    );
    assert_eq!(
        document
            .pointer("/nodes/1/attributes/0/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "raw attribute is directly filterable"
    );
    assert!(
        document.pointer("/nodes/1/attributes/0/position").is_some(),
        "raw attribute retains the exact original coordinate"
    );
    Ok(())
}

fn verify_values(entity: &Value) -> TestResult {
    let segment = entity
        .pointer("/areas/0/segments/0")
        .ok_or("original segment")?;
    let translation = serde_json::json!([4.0, 5.0, 6.0]);
    assert_eq!(
        segment.pointer("/localTransform/value/translation"),
        Some(&translation),
        "original segment translation"
    );
    assert_eq!(
        segment.pointer("/geometry/type").and_then(Value::as_str),
        Some("box"),
        "original shape classification"
    );
    assert_eq!(
        entity.pointer("/components/0/kind").and_then(Value::as_str),
        Some("door"),
        "authored door role"
    );
    let nested = entity
        .pointer("/components/1/loadouts/1")
        .ok_or("nested loadout")?;
    assert_eq!(
        nested.get("parentEntry").and_then(Value::as_u64),
        Some(0),
        "nested override refers to the original parent entry"
    );
    assert!(
        nested
            .pointer("/groups/0/entries")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "explicitly empty manual override"
    );
    Ok(())
}

#[test]
fn authored_soc_package_is_staged_and_replaceable_as_one_package() -> TestResult {
    let (folder, archive, container) = fixture()?;
    let layers = [&archive, &container];
    let capture = SocContainerCapture::read(&layers, &CancelToken::new())?;
    let soc = SocScene::from_capture(&capture)?;
    let interior = Interior::build(&layers, "selected.socpak", &InteriorOptions::default())?;
    let plain = InteriorScene::new("selected", &interior);
    let with_soc = plain.with_authored_soc(&soc);
    let model = ArchiveModels::new(&archive);
    let options = GlbOptions::default();
    let info = PackageInfo::new(None, None);
    let target = folder.path().join("package");
    let write = |scene| {
        write_interior_package(
            scene,
            &model,
            &model.textures(),
            &options,
            &info,
            &target,
            &|| false,
        )
    };
    let manifest_path = target.join("scene.json");
    let baseline = write(&plain)?;
    let read_manifest =
        || -> TestResult<Value> { Ok(serde_json::from_slice(&std::fs::read(&manifest_path)?)?) };
    let baseline_metadata_bytes = package_metadata_bytes(&target)?;
    let before = read_manifest()?;
    assert!(
        before.get("authored_soc").is_none(),
        "only explicit output requests add the sidecar"
    );
    let authored = write(&with_soc)?;
    let manifest = read_manifest()?;
    assert_eq!(
        manifest.get("nodes"),
        before.get("nodes"),
        "the visible scene is identical"
    );
    let relative = manifest
        .pointer("/authored_soc/file")
        .and_then(Value::as_str)
        .ok_or("companion reference")?;
    let sidecar = target.join(relative);
    let bytes = std::fs::read(&sidecar)?;
    let sidecar_len = u64::try_from(bytes.len())?;
    verify_package_schemas(&bytes, &manifest)?;
    let authored_metadata_bytes = package_metadata_bytes(&target)?;
    verify_receipt_member(&target, relative)?;
    assert_eq!(
        authored.files,
        baseline.files + 1,
        "one companion file is counted"
    );
    assert_eq!(
        authored.bytes + baseline_metadata_bytes,
        baseline.bytes + authored_metadata_bytes + sidecar_len,
        "companion bytes are counted"
    );
    verify_package_replacement(&target, &sidecar, || write(&plain), || write(&with_soc))
}

fn package_metadata_bytes(root: &Path) -> TestResult<u64> {
    Ok(std::fs::metadata(root.join("scene.json"))?.len()
        + std::fs::metadata(root.join("receipt.json"))?.len())
}

fn verify_receipt_member(root: &Path, relative: &str) -> TestResult {
    let receipt: Value = serde_json::from_slice(&std::fs::read(root.join("receipt.json"))?)?;
    let files = receipt
        .get("files")
        .and_then(Value::as_array)
        .ok_or("receipt files")?;
    assert!(
        files
            .iter()
            .any(|row| row.get("path").and_then(Value::as_str) == Some(relative)),
        "the receipt covers the declared companion"
    );
    Ok(())
}

fn verify_package_schemas(bytes: &[u8], manifest: &Value) -> TestResult {
    let decoded: Value = serde_json::from_slice(bytes)?;
    let validator = jsonschema::validator_for(&soc_scene_schema())?;
    validator
        .validate(&decoded)
        .map_err(|error| error.to_string())?;
    let validator = jsonschema::validator_for(&manifest_schema())?;
    validator
        .validate(manifest)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn verify_package_replacement(
    target: &Path,
    sidecar: &Path,
    plain: impl Fn() -> Result<p4kx_blender::PackageSummary, PackageError>,
    authored: impl FnOnce() -> Result<p4kx_blender::PackageSummary, PackageError>,
) -> TestResult {
    authored()?;
    let unrelated = target.join("authored/unrelated.txt");
    std::fs::write(&unrelated, b"unrelated")?;
    assert!(
        matches!(plain(), Err(PackageError::NotAPackage { .. })),
        "foreign sidecar files forbid replacement"
    );
    assert!(unrelated.is_file(), "the foreign file is preserved");
    std::fs::remove_file(unrelated)?;
    plain()?;
    assert!(
        !sidecar.exists(),
        "replacement retires the old optional companion"
    );
    Ok(())
}

#[test]
fn checked_in_soc_scene_schema_is_current_and_valid() -> TestResult {
    let schema = soc_scene_schema();
    jsonschema::draft202012::meta::validate(&schema).map_err(|error| error.to_string())?;
    let mut generated = serde_json::to_string_pretty(&schema)?;
    generated.push('\n');
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/soc-scene.schema.json");
    if std::env::var_os("P4KX_BLESS").is_some_and(|value| value == "1") {
        std::fs::write(&path, &generated)?;
    }
    assert!(
        std::fs::read_to_string(path)? == generated,
        "run with P4KX_BLESS=1 to regenerate the SOC scene schema"
    );
    Ok(())
}
