// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx entity glb` of a creature that names an animation database writes its clips only when
//! `--animations` is given, and lists what it could not write.

use std::error::Error;
use std::path::Path;

use p4kx_testkit::{
    CREATURE_ANIMATED, CREATURE_ANIMATION_DATABASE_LOST, CREATURE_ANIMATION_SCOPED, check_glb,
    creature_entries,
};
use serde_json::Value;

use crate::entity_package::write_archive;

type TestResult = Result<(), Box<dyn Error>>;

fn glb(archive: &Path, class: &str, out: &Path, extra: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .args(["entity", "glb"])
        .arg(archive)
        .arg(class)
        .arg("--out")
        .arg(out)
        .args(extra)
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}

#[test]
fn the_clips_of_a_creature_are_written_and_the_report_counts_them() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), creature_entries())?;
    let out = dir.path().join("creature.glb");

    let text = glb(&archive, CREATURE_ANIMATED, &out, &["--animations"])?;

    let checked = check_glb(&std::fs::read(&out)?)?;
    assert_eq!(checked.document.animations().len(), 1);
    assert!(text.contains("1 animations, 1 channels"), "{text}");
    assert!(text.contains("additive"), "{text}");
    Ok(())
}

#[test]
fn the_json_report_lists_the_written_animations_and_the_ones_left_out() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), creature_entries())?;
    let out = dir.path().join("creature.glb");

    let text = glb(
        &archive,
        CREATURE_ANIMATED,
        &out,
        &["--animations", "--json"],
    );
    let report: Value = serde_json::from_str(&text?)?;

    assert_eq!(report["animations"][0]["name"], "wave");
    assert_eq!(report["animations"][0]["channels"], 1);
    let left_out = report["issues"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|issue| issue["kind"].as_str())
        .filter(|kind| *kind == "animation_not_used")
        .count();
    assert_eq!(left_out, 2, "{report}");
    assert_eq!(report["skipped"], 0, "animations never skip a part");
    Ok(())
}

#[test]
fn the_report_binds_nested_selectors_to_the_selected_physical_clips() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), creature_entries())?;
    let out = dir.path().join("scoped.glb");
    let output = glb(
        &archive,
        CREATURE_ANIMATION_SCOPED,
        &out,
        &["--animations", "--json"],
    )?;
    let report: Value = serde_json::from_str(&output)?;
    let sources = &report["animation_sources"];
    assert_eq!(sources["selection_complete"], true, "{sources}");
    for field in ["database", "geometry", "skeleton", "parameters"] {
        assert_eq!(
            sources["owner"][field]["sourceSetComplete"], true,
            "{sources}"
        );
    }
    let clips = sources["clips"].as_array().ok_or("selected clips")?;
    let [dba, caf] = clips.as_slice() else {
        return Err("expected one in-scope CAF and one in-scope DBA clip".into());
    };
    assert_eq!(dba["ordinal"]["value"], 1);
    assert_eq!(caf["ordinal"]["value"], 0);
    assert!(
        dba["entry"]["value"]
            .as_str()
            .is_some_and(|path| path.ends_with("scoped.dba"))
    );
    assert!(
        caf["entry"]["value"]
            .as_str()
            .is_some_and(|path| path.ends_with("wave.caf"))
    );
    let selectors = dba["graph_selectors"].as_array().ok_or("ADB selectors")?;
    assert_eq!(
        selectors
            .iter()
            .map(|value| &value["value"])
            .collect::<Vec<_>>(),
        ["scoped", "SCOPED"]
    );
    for selector in selectors {
        assert_eq!(selector["sourceSetComplete"], true, "{selector}");
        assert_eq!(selector["containsEncryptedSource"], false, "{selector}");
        assert_graph_uses(sources, selector)?;
    }
    Ok(())
}

fn assert_graph_uses(sources: &Value, selector: &Value) -> TestResult {
    let refs = selector["sourceUsesRefs"]
        .as_array()
        .ok_or("field use references")?;
    let uses = sources
        .get("source_inventory")
        .and_then(|inventory| inventory.get("uses"))
        .ok_or("source use inventory")?
        .as_array()
        .ok_or("source uses")?;
    let fields: Vec<_> = uses
        .iter()
        .filter(|usage| refs.contains(&usage["id"]))
        .filter_map(|usage| usage.get("location")?.get("field")?.as_str())
        .collect();
    assert!(
        fields
            .iter()
            .any(|field| field.ends_with(".animDatabase3P")),
        "missing animated-character selector among {fields:?}"
    );
    for field in ["File", "name", "animationSources", "databasePath"] {
        assert!(fields.contains(&field), "missing {field} among {fields:?}");
    }
    Ok(())
}

#[test]
fn without_the_flag_the_creature_is_written_static_and_nothing_is_read() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), creature_entries())?;
    let out = dir.path().join("creature.glb");

    let text = glb(&archive, CREATURE_ANIMATED, &out, &[])?;

    let checked = check_glb(&std::fs::read(&out)?)?;
    assert_eq!(checked.document.animations().len(), 0);
    assert!(
        !text.contains(" channels") && !text.contains("additive"),
        "{text}"
    );
    Ok(())
}

#[test]
fn a_database_the_archive_lacks_is_a_note_and_the_creature_is_still_written() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), creature_entries())?;
    let out = dir.path().join("creature.glb");

    let text = glb(
        &archive,
        CREATURE_ANIMATION_DATABASE_LOST,
        &out,
        &["--animations"],
    )?;

    let checked = check_glb(&std::fs::read(&out)?)?;
    assert_eq!(checked.document.animations().len(), 0);
    assert!(checked.document.skins().len() > 0, "{text}");
    assert!(
        text.contains("note Test_Creature_AnimationDatabaseLost: the animations are not written"),
        "{text}"
    );
    Ok(())
}
