// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Kept results report the key use of their build, and are built again once the key is there.

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestEntry, TestLoadout, armour_xml_loadout, entity_schema,
    test_key,
};
use serde_json::{Value, json};

use crate::archive_key_note::{hint_of, install as audio_install, search, texts};
use crate::support::{Install, Server, TestResult};

/// An Archive whose Entity `TEST_Grunt` has an XML Loadout stored encrypted, so a Loadout tree
/// resolved without the key lacks the ports that file names.
fn loadout_install() -> TestResult<Install> {
    let (path, bytes) = armour_xml_loadout();
    let forge = [
        TestEntity::new(0x10, "TEST_Torso", "entities/scitem/torso.xml"),
        TestEntity::new(0x11, "TEST_Helmet", "entities/scitem/helmet.xml"),
        TestEntity::new(0x21, "TEST_Grunt", "entities/scitem/grunt.xml")
            .loadout(TestLoadout::Xml(path.clone())),
    ]
    .iter()
    .fold(entity_schema(DataForgeBuilder::new()), |forge, entity| {
        forge.record(entity.record())
    });
    Install::new(vec![
        TestEntry::zstd("Data/Game2.dcb", forge.build()),
        TestEntry::stored(format!("Data\\{}", path.replace('/', "\\")), bytes).encrypted(),
    ])
}

fn loadout(server: &mut Server) -> TestResult<Value> {
    server.call("entity_loadout", &json!({ "entity": "TEST_Grunt" }))
}

fn store_key(install: &Install) -> TestResult {
    std::fs::write(
        install.config().join("archive.key"),
        format!("{}\n", test_key().to_hex()),
    )?;
    Ok(())
}

fn assert_unread(result: &Value, unread: u64) {
    assert_eq!(
        result.pointer("/_meta/archive_key"),
        Some(&json!("missing")),
        "{result}"
    );
    assert_eq!(
        result.pointer("/_meta/encrypted_entries_unread"),
        Some(&json!(unread)),
        "{result}"
    );
    assert!(hint_of(result, "encrypted Entr").is_some(), "{result}");
}

fn assert_complete(result: &Value) {
    assert_eq!(
        result.pointer("/_meta/archive_key"),
        Some(&json!("given")),
        "{result}"
    );
    assert_eq!(
        result.pointer("/_meta/encrypted_entries_unread"),
        Some(&json!(0)),
        "{result}"
    );
    assert_eq!(texts(result).len(), 1, "only the JSON text: {result}");
}

#[test]
fn a_reused_audio_index_reports_the_unread_entries_of_its_build() -> TestResult {
    let install = audio_install()?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;

    let first = search(&mut server)?;
    let second = search(&mut server)?;

    assert_unread(&first, 1);
    assert_unread(&second, 1);
    assert_eq!(texts(&first), texts(&second), "the same note");
    Ok(())
}

#[test]
fn an_audio_index_built_without_the_key_is_built_again_once_it_is_stored() -> TestResult {
    let install = audio_install()?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;
    assert_unread(&search(&mut server)?, 1);

    store_key(&install)?;
    let after = search(&mut server)?;

    assert_complete(&after);
    assert_eq!(
        after["structuredContent"]["total"], 1,
        "the encrypted definition is read now: {after}"
    );
    let again = search(&mut server)?;
    assert_complete(&again);
    Ok(())
}

#[test]
fn a_reused_audio_index_built_with_the_key_stays_complete() -> TestResult {
    let install = audio_install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let first = search(&mut server)?;
    let second = search(&mut server)?;

    assert_complete(&first);
    assert_complete(&second);
    Ok(())
}

#[test]
fn a_reused_loadout_tree_reports_the_unread_entries_of_its_resolution() -> TestResult {
    let install = loadout_install()?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;

    let first = loadout(&mut server)?;
    let second = loadout(&mut server)?;

    assert_unread(&first, 1);
    assert_unread(&second, 1);
    assert_eq!(texts(&first), texts(&second), "the same note");
    Ok(())
}

#[test]
fn a_loadout_tree_resolved_without_the_key_is_resolved_again_once_it_is_stored() -> TestResult {
    let install = loadout_install()?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;
    let first = loadout(&mut server)?;
    assert_unread(&first, 1);

    store_key(&install)?;
    let after = loadout(&mut server)?;

    assert_complete(&after);
    assert!(
        after["structuredContent"]["total"].as_u64() > first["structuredContent"]["total"].as_u64(),
        "the ports of the encrypted XML Loadout are there now: {after}"
    );
    assert_complete(&loadout(&mut server)?);
    Ok(())
}

#[test]
fn a_reused_loadout_tree_resolved_with_the_key_stays_complete() -> TestResult {
    let install = loadout_install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let first = loadout(&mut server)?;
    let second = loadout(&mut server)?;

    assert_complete(&first);
    assert_complete(&second);
    Ok(())
}
