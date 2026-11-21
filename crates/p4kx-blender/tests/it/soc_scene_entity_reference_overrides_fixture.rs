// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use base64::engine::general_purpose::STANDARD;

use p4kx_assembly::{ArchiveModels, Interior, InteriorOptions, SocContainerCapture};
use p4kx_blender::{InteriorScene, PackageInfo, SocScene, write_interior_package};
use p4kx_extraction::{Archive, CancelToken, GlbOptions};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestChunkFile, TestContainer, TestElement,
    TestEntry,
};
use serde_json::{Value, json};

use super::{CONTAINER, OTHER, RAW_SOURCES, ROOT, TestResult, array, at};

pub(super) fn authored(owner: &str) -> Vec<u8> {
    cryxml(
        &TestElement::new(ROOT)
            .child(TestElement::new("Extra").content("original unknown text"))
            .child(
                TestElement::new("Entity")
                    .attribute("CryGUID", owner)
                    .child(TestElement::new("Extra"))
                    .child(
                        TestElement::new("AutoSpawnOverrides")
                            .child(TestElement::new("Extra"))
                            .child(
                                TestElement::new("AutoSpawnOverrideElement")
                                    .attribute("AutoSpawnIdx", "0007")
                                    .attribute("Target", "raw,, chain")
                                    .attribute("AA", OTHER),
                            ),
                    ),
            ),
    )
}

pub(super) fn repeated(owner: &str) -> Vec<u8> {
    cryxml(
        &TestElement::new(ROOT).child(
            TestElement::new("Entity")
                .attribute("CryGUID", owner)
                .child(
                    TestElement::new("AutoSpawnOverrides").child(
                        TestElement::new("AutoSpawnOverrideElement")
                            .attribute("Target", "")
                            .attribute("AA", "first")
                            .attribute("AA", "second")
                            .attribute("Extra", "retained"),
                    ),
                )
                .child(TestElement::new("AutoSpawnOverrides")),
        ),
    )
}

fn cryxml(element: &TestElement) -> Vec<u8> {
    element.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

pub(super) fn container<const N: usize>(owners: [&str; N]) -> TestContainer {
    TestContainer::new()
        .entry(descriptor())
        .entry(placed(owners))
}

pub(super) fn placed<const N: usize>(owners: [&str; N]) -> TestEntry {
    let xml = owners
        .into_iter()
        .fold(TestElement::new("Entities"), |root, guid| {
            root.child(TestElement::new("Entity").attribute("EntityCryGUID", guid))
        });
    TestEntry::stored(
        "placed.soc",
        TestChunkFile::crch().chunk(4, 3, cryxml(&xml)).bytes(),
    )
}

pub(super) fn open(
    bytes: Vec<u8>,
    encrypted: bool,
) -> TestResult<(tempfile::TempDir, Archive, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("scene.p4k");
    let root = TestEntry::stored(CONTAINER, bytes);
    let root = if encrypted { root.encrypted() } else { root };
    ArchiveBuilder::new().entry(root).write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = archive.entries().first().ok_or("physical container")?;
    let inner = archive.open_container(entry)?;
    Ok((folder, archive, inner))
}

pub(super) fn scene(bytes: Vec<u8>, encrypted: bool) -> TestResult<Value> {
    let (_folder, archive, inner) = open(bytes, encrypted)?;
    let capture = SocContainerCapture::read(&[&archive, &inner], &CancelToken::new())?;
    Ok(serde_json::to_value(SocScene::from_capture(&capture)?)?)
}

pub(super) fn package_value(
    folder: &Path,
    archive: &Archive,
    inner: &Archive,
    scene: &SocScene,
) -> TestResult<Value> {
    let interior = Interior::build(&[archive, inner], CONTAINER, &InteriorOptions::default())?;
    let assembled = InteriorScene::new("overrides", &interior).with_authored_soc(scene);
    let model = ArchiveModels::new(archive);
    let target = folder.join("package");
    write_interior_package(
        &assembled,
        &model,
        &model.textures(),
        &GlbOptions::default(),
        &PackageInfo::new(None, None),
        &target,
        &|| false,
    )?;
    let manifest: Value = serde_json::from_slice(&std::fs::read(target.join("scene.json"))?)?;
    let path = at(&manifest, "/authored_soc/file")?
        .as_str()
        .ok_or("SOC sidecar path")?;
    Ok(serde_json::from_slice(&std::fs::read(target.join(path))?)?)
}

pub(super) fn descriptor() -> TestEntry {
    TestEntry::stored(
        "overrides.xml",
        br#"<ObjectContainer><StreamingOCs><OC name="placed.soc"/></StreamingOCs></ObjectContainer>"#,
    )
}

pub(super) fn verify_raw(
    scene: &Value,
    member: &Value,
    expected: &[u8],
    encrypted: Option<bool>,
) -> TestResult {
    use base64::Engine as _;

    let source = at(member, "/source")?;
    let mut rows = array(scene, RAW_SOURCES)?
        .iter()
        .filter(|row| row.get("source") == Some(source));
    let row = rows.next().ok_or("verified physical raw payload")?;
    assert!(rows.next().is_none(), "one raw payload per physical source");
    let raw = at(row, "/raw")?;
    let encoded = at(raw, "/value")?.as_str().ok_or("base64 payload")?;
    assert_eq!(
        encoded,
        STANDARD.encode(expected),
        "unaltered payload in standard padded Base64"
    );
    assert_eq!(
        at(raw, "/containsEncryptedSource")?,
        &encrypted.map_or(Value::Null, Value::Bool),
        "raw bytes use actual payload encryption"
    );
    assert_eq!(
        at(raw, "/sourceSetComplete")?,
        true,
        "verified source proof is complete"
    );
    let [id] = array(raw, "/sourceUsesRefs")? else {
        return Err("exactly one whole member payload use".into());
    };
    let location = json!({
        "type": "subresource",
        "namespace": "socEntityReferenceOverride",
        "indices": ["0", expected.len().to_string()],
        "field": "raw",
    });
    assert!(
        array(scene, "/sourceInventory/uses")?.iter().any(|used| {
            used.get("id") == Some(id)
                && used.get("source") == Some(source)
                && used.get("location") == Some(&location)
                && used.get("scope").and_then(Value::as_str) == Some("payload")
                && used.get("role").and_then(Value::as_str) == Some("payload")
        }),
        "exact decoded byte span of the correct physical source"
    );
    Ok(())
}
