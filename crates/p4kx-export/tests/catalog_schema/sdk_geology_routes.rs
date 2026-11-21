// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::terrain::{
    CatalogJsonNumber, GeologySet, LooseArchivePathState, LooseDefinitionSelection as Selection,
    TerrainGuidLinkState, read_geology_sets, read_geology_texture_sets,
};
use p4kx_extraction::Archive;
use p4kx_testkit::TestEntry;
use serde_json::Value;

use super::{Route, TestResult, open, parity};

pub(super) const SETS: &str = "/definitions/geologySets";
pub(super) const TEXTURE_SETS: &str = "/definitions/geologyTextureSets";
const SET: &str = "Data/Libs/Planets/geology_sets/fixture.geology";
const DAMAGED_SET: &str = "Data/Libs/Planets/geology_sets/damaged.geology";
const TEXTURE_SET: &str = "Data/Libs/Planets/geology_sets/fixture.geotextureset";
const ASSET: &str = "Data/Libs/Planets/geology_assets/fixture.geoasset";
const MEDIA: &str = "Data/Textures/fixture/terrain.dds";
const SET_GUID: &str = "dddddddd-0000-0000-0000-000000000101";
const TEXTURE_GUID: &str = "dddddddd-0000-0000-0000-000000000102";
const ASSET_GUID: &str = "dddddddd-0000-0000-0000-000000000103";

pub(super) fn read(archive: &Archive, catalog: &Value) -> TestResult<[Route; 2]> {
    let sets = read_geology_sets(archive, Selection::ArchivePath(SET), None)?;
    let [set] = sets.rows.as_slice() else {
        return Err("one selected geology set".into());
    };
    verify_set(set)?;
    let textures =
        read_geology_texture_sets(archive, Selection::Guid(TEXTURE_GUID.parse()?), None)?;
    let [texture_set] = textures.rows.as_slice() else {
        return Err("one selected geology texture set".into());
    };
    assert_eq!(
        texture_set
            .min_roughness
            .value()
            .map(CatalogJsonNumber::as_str),
        Some("0.4500"),
        "the texture set retains the authored numeric spelling"
    );
    let paths = texture_set
        .biplanar_textures
        .as_ref()
        .ok_or("texture paths")?;
    let [layer] = paths.slots.as_slice() else {
        return Err("one authored biplanar texture path".into());
    };
    assert_eq!(
        layer.target.state().value(),
        Some(&LooseArchivePathState::Resolved),
        "the encrypted texture set links to the unreadable medium by directory"
    );
    Ok([
        parity(catalog, SETS, sets.availability, &sets.rows)?,
        parity(catalog, TEXTURE_SETS, textures.availability, &textures.rows)?,
    ])
}

fn verify_set(set: &GeologySet) -> TestResult {
    let links = set.links.as_ref().ok_or("authored geology links")?;
    let slots = links.scattered_assets.as_deref().ok_or("original slots")?;
    let [resolved, explicit_null, wrong_family] = slots else {
        return Err("three original asset slots".into());
    };
    assert_eq!(
        [
            resolved.ordinal.get(),
            explicit_null.ordinal.get(),
            wrong_family.ordinal.get()
        ],
        [0, 1, 2],
        "the authored slot coordinates survive"
    );
    assert_eq!(
        resolved.link.state.value(),
        Some(&TerrainGuidLinkState::Resolved),
        "the first asset resolves to its authored geology definition"
    );
    assert_eq!(
        explicit_null.link.state.value(),
        Some(&TerrainGuidLinkState::Null),
        "the second original slot remains null"
    );
    assert_eq!(
        wrong_family.link.state.value(),
        Some(&TerrainGuidLinkState::WrongFamily),
        "a texture set cannot resolve an asset slot"
    );
    assert_eq!(
        links.geology_texture_set.state.value(),
        Some(&TerrainGuidLinkState::Resolved),
        "the texture set resolves for its dedicated selector"
    );
    let source = set
        .scattered_assets
        .value()
        .and_then(|slots| slots.first())
        .ok_or("first original asset GUID")?;
    assert_eq!(
        source.origin().contains_encrypted_source(),
        Some(false),
        "the original selector comes from the clear geology set"
    );
    assert_eq!(
        resolved.link.state.origin().contains_encrypted_source(),
        Some(true),
        "the selected relation cites the actual encrypted asset identity"
    );
    Ok(())
}

#[test]
fn geology_owner_selection_and_texture_directory_need_no_dataforge() -> TestResult {
    let (_folder, archive) = open(definitions().into_iter().chain([
        TestEntry::stored(DAMAGED_SET, b"unreadable unrelated geology owner").with_crc32c(0),
        TestEntry::stored(MEDIA, b"unreadable texture bytes").with_crc32c(0),
    ]))?;
    let stored_path = SET.replace('/', "\\");
    let index = archive
        .entries()
        .iter()
        .find(|entry| entry.path().eq_ignore_ascii_case(&stored_path))
        .ok_or("physical geology Entry index")?
        .index();
    let exact = read_geology_sets(&archive, Selection::EntryIndex(u32::try_from(index)?), None)?;
    assert!(
        exact.is_selection_complete(),
        "the damaged owner is outside the exact selection"
    );
    let [row] = exact.rows.as_slice() else {
        return Err("one exact geology set".into());
    };
    verify_set(row)?;
    let guid = read_geology_sets(&archive, Selection::Guid(SET_GUID.parse()?), None)?;
    assert_eq!(guid.entries.failed_selection_candidates, 1);
    assert!(
        !guid.is_selection_complete(),
        "the undecidable GUID owner remains visible"
    );
    let textures = read_geology_texture_sets(&archive, Selection::ArchivePath(TEXTURE_SET), None)?;
    assert!(
        textures.is_selection_complete(),
        "the texture header and directory are enough"
    );
    Ok(())
}

pub(super) fn definitions() -> [TestEntry; 3] {
    let set = format!(
        r#"{{"GUID":"{SET_GUID}","scatteredAssets":["{ASSET_GUID}",null,"{TEXTURE_GUID}"],"cliffAssets":[],"geologyTextureSetGUID":"{TEXTURE_GUID}"}}"#
    );
    let texture = format!(
        r#"{{"GUID":"{TEXTURE_GUID}","biplanarLayers":[{{"textureName":"{MEDIA}"}}],"fMinRoughness":0.4500}}"#
    );
    [
        TestEntry::stored(SET, set),
        TestEntry::stored(TEXTURE_SET, texture).encrypted(),
        TestEntry::stored(ASSET, format!(r#"{{"GUID":"{ASSET_GUID}"}}"#)).encrypted(),
    ]
}
