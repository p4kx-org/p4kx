// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use super::field_origin::{LITERAL, main_system, normal_catalog, source, system, target};
use super::{ALIAS_TARGET, ROOT};
use crate::health_origin_assertions as proof;
use crate::health_origin_fixture::TestResult;

const OTHER_ROOT: &str = "Data/ObjectContainers/PU/system/Other/OtherSystem.socpak";
const SHARED: &str = "Data/ObjectContainers/PU/synthetic/shared-placement.socpak";
const FALLBACK: &str = "Data/ObjectContainers/PU/system/FallBack/loose.socpak";

#[test]
fn both_systems_keep_their_own_directory_source_for_a_shared_loot_seed() -> TestResult {
    let fixture = source(
        [
            system_root(ROOT, "syntheticsystem.xml"),
            system_root(OTHER_ROOT, "othersystem.xml").encrypted(),
            system(SHARED, "shared-placement.xml", LITERAL).encrypted(),
        ],
        [target()],
    )?;
    let catalog = normal_catalog(&fixture)?;
    let [other, first] = proof::rows(&catalog, "/lootContainers")? else {
        return Err("two system contexts for the same physical configured placement".into());
    };
    for (row, value, path) in [(other, "other", OTHER_ROOT), (first, "synthetic", ROOT)] {
        proof::present(row, "container", &json!(ALIAS_TARGET), Some(true))?;
        verify_system(&catalog, row, value, path)?;
    }
    proof::closure(&catalog)
}

#[test]
fn an_unreached_seed_uses_its_own_physical_system_directory() -> TestResult {
    let fixture = source(
        [
            main_system(LITERAL),
            system(FALLBACK, "loose.xml", LITERAL).encrypted(),
        ],
        [target()],
    )?;
    let catalog = normal_catalog(&fixture)?;
    let [fallback, main] = proof::rows(&catalog, "/lootContainers")? else {
        return Err("one unreached fallback and one main system occurrence".into());
    };
    proof::present(fallback, "container", &json!(ALIAS_TARGET), Some(true))?;
    verify_system(&catalog, fallback, "fallback", FALLBACK)?;
    verify_system(&catalog, main, "synthetic", ROOT)?;
    proof::closure(&catalog)
}

fn system_root(path: &str, descriptor: &str) -> TestEntry {
    let xml = format!(
        r#"<ObjectContainer version="14"><ChildObjectContainers>
            <Child name="{SHARED}" external="1" pos="0,0,0"/>
            </ChildObjectContainers></ObjectContainer>"#
    );
    TestEntry::stored(
        path,
        ArchiveBuilder::v2()
            .entry(TestEntry::stored(descriptor, xml))
            .build(),
    )
}

fn verify_system(catalog: &Value, row: &Value, value: &str, path: &str) -> TestResult {
    let system = proof::present(row, "system", &json!(value), Some(false))?;
    let uses = proof::uses(catalog, system)?;
    assert!(
        uses.iter().any(|used| used["role"] == "selector"
            && used.pointer("/location/type") == Some(&json!("archiveDirectoryPaths"))),
        "the system choice cites the actual archive directory survey"
    );
    for role in ["payload", "conversion"] {
        let matching = uses.iter().filter(|used| {
            used["role"] == role && used.pointer("/location/field") == Some(&json!("path"))
        });
        let matching: Vec<_> = matching.copied().collect();
        let [directory_use] = matching.as_slice() else {
            return Err(format!("one actual {role} directory use").into());
        };
        assert_eq!(
            directory_use["scope"], "archiveDirectoryMetadata",
            "the folder is derived from the directory, including for encrypted Entry payloads"
        );
        assert_eq!(
            proof::source(catalog, directory_use)?.pointer("/path/value"),
            Some(&json!(path)),
            "the retained physical path belongs to this system context"
        );
    }
    Ok(())
}
