// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_testkit::{
    ArchiveBuilder, MOUNT_BONE, SATCHEL_HELPER_Z, check_glb, mounted_overlay_entries,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

fn run(archive: &Path, out: &Path, wearer: &str, pieces: &[&str]) -> Result<Value, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command
        .args(["--json", "armour", "glb"])
        .arg(archive)
        .args(["--loadout", wearer, "--body", "male", "--out"])
        .arg(out);
    for piece in pieces {
        command.args(["--piece", piece]);
    }
    let result = command.output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(serde_json::from_slice(&result.stdout)?)
}

fn fixture(path: &Path) -> TestResult {
    mounted_overlay_entries()
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(path)?;
    Ok(())
}

fn assert_mount(out: &Path) -> TestResult {
    let glb = check_glb(&std::fs::read(out)?)?;
    let parent = glb
        .document
        .nodes()
        .find(|node| node.children().any(|child| child.name() == Some("satchel")))
        .ok_or("the rigid part has a bone parent")?;
    assert_eq!(
        parent.name(),
        Some(MOUNT_BONE),
        "the rigid part follows the current holder's bone"
    );
    let child = parent
        .children()
        .find(|child| child.name() == Some("satchel"))
        .ok_or("the rigid part")?;
    let (translation, _, _) = child.transform().decomposed();
    for (actual, expected) in translation
        .into_iter()
        .zip([0.0, 0.6, -SATCHEL_HELPER_Z + 0.8])
    {
        assert!(
            (actual - expected).abs() < 1e-5,
            "current port offset {translation:?}"
        );
    }
    Ok(())
}

#[test]
fn a_new_explicit_rigid_piece_uses_the_unique_free_current_port() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    fixture(&archive)?;
    for (label, pieces) in [
        ("retained", &["Mount_Explicit_Satchel"][..]),
        (
            "replaced",
            &["Mount_Explicit_Satchel", "Mount_New_Carrier"][..],
        ),
    ] {
        let out = directory.path().join(format!("{label}.glb"));
        let report = run(&archive, &out, "Mount_Free_Wearer", pieces)?;
        assert!(
            report["parts"]
                .as_array()
                .ok_or("parts")?
                .iter()
                .any(|part| part["name"] == "Mount_Explicit_Satchel"),
            "new piece is drawn in both normal branches: {report}"
        );
        assert_mount(&out)?;
    }
    Ok(())
}

#[test]
fn new_pieces_never_claim_occupied_ambiguous_or_unproven_ports() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    fixture(&archive)?;
    for (wearer, pieces, count, reason) in [
        (
            "Mount_Occupied_Wearer",
            &["Mount_Explicit_Satchel"][..],
            1,
            "occupied",
        ),
        (
            "Mount_Ambiguous_Wearer",
            &["Mount_Explicit_Satchel"][..],
            1,
            "competing",
        ),
        (
            "Mount_Unknown_Wearer",
            &["Mount_Explicit_Satchel"][..],
            1,
            "unproven tag requirements",
        ),
        (
            "Mount_Free_Wearer",
            &["Mount_Explicit_Satchel", "Mount_Second_Satchel"][..],
            2,
            "claimed more than once",
        ),
    ] {
        let out = directory.path().join(format!("{wearer}.glb"));
        let report = run(&archive, &out, wearer, pieces)?;
        let glb = check_glb(&std::fs::read(&out)?)?;
        assert!(
            glb.document
                .nodes()
                .all(|node| node.name() != Some("satchel")),
            "an unproven mount cannot place a rigid mesh at the origin"
        );
        let issues: Vec<_> = report["issues"]
            .as_array()
            .ok_or("issues")?
            .iter()
            .filter(|issue| issue["kind"] == "worn_mount_unresolved")
            .collect();
        assert_eq!(issues.len(), count, "{report}");
        assert!(
            issues.iter().all(|issue| issue["skipped"] == true
                && issue["reason"]
                    .as_str()
                    .is_some_and(|text| text.contains(reason))),
            "{report}"
        );
    }
    Ok(())
}
