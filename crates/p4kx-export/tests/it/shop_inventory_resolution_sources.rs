// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use serde_json::Value;

use super::{
    CASES, CONTAINER, INVENTORIES, METADATA, PREFAB, REGISTRY, SOC, TestResult, entity, key,
    original,
};

pub(super) fn assert_original_sources(archive: &Archive) -> TestResult {
    let xml = original(archive, REGISTRY)?;
    let registry = roxmltree::Document::parse(&xml)?;
    for index in 0..CASES.len() {
        let key = key(index);
        assert!(
            registry.descendants().any(|node| {
                node.has_tag_name("ShopBackup") && node.attribute("ID") == Some(key.as_str())
            }),
            "the real registry declares {key}"
        );
    }
    let xml = original(archive, PREFAB)?;
    let prefab = roxmltree::Document::parse(&xml)?;
    for path in CASES.iter().flat_map(|case| case.paths.iter()) {
        assert!(
            prefab.descendants().any(|node| {
                node.has_tag_name("EntityComponentShop")
                    && node.attributes().any(|field| field.value() == *path)
            }),
            "the real prefab authored {path}"
        );
    }
    let socpak = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == CONTAINER)
        .ok_or("actual shop container")?;
    let container = archive.open_container(socpak)?;
    let xml = original(&container, METADATA)?;
    let metadata = roxmltree::Document::parse(&xml)?;
    for index in [5, 6] {
        let guid = entity(index);
        assert!(
            metadata.descendants().any(|node| {
                node.has_tag_name("Entity") && node.attribute("CryGUID") == Some(guid.as_str())
            }),
            "the real container lists {guid}"
        );
    }
    let soc = original(&container, SOC)?;
    assert_eq!(
        soc,
        format!("CrChF\0{}\0EntityComponentShop\0", entity(6)),
        "only the actual no-file Entity has a readable anchored component"
    );
    let unreadable = original(archive, &format!("{INVENTORIES}Inv_Unreadable.json"))?;
    assert!(
        serde_json::from_str::<Value>(&unreadable).is_err(),
        "the actual target is invalid JSON"
    );
    assert!(
        archive
            .entries()
            .iter()
            .all(|entry| !entry.path().ends_with("Inv_Missing.json")),
        "the actual archive has no selected missing target"
    );
    Ok(())
}
