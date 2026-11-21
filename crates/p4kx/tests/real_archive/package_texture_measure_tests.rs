// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::time::Duration;

use serde_json::{Value, json};

use super::{Built, Check, verify};
use crate::package_fault;

const SOURCE: &str = "Objects/Test/Shared.dds";

#[test]
fn repeated_archive_source_counts_once_across_distinct_renditions() -> package_fault::Result<()> {
    let (directory, mut built) = fixture()?;
    built.manifest["materials"] = json!([{"textures": [{"texture": "one"}, {"texture": "two"}]}]);
    built.manifest["meshes"] = json!([{"materials": [{"textures": {"specular": "one"}}]}]);
    assert_eq!(
        verify(&built)?,
        1,
        "the canary floor measures physical sources"
    );
    directory.close()?;
    Ok(())
}

#[test]
fn duplicate_renditions_files_and_ids_fail_independently() -> package_fault::Result<()> {
    let (directory, mut built) = fixture()?;
    for (field, duplicate, expected) in [
        (
            "source_rendition",
            "specular",
            Check::TextureDuplicateRendition,
        ),
        ("file", "TEXTURES/ONE.PNG", Check::TextureDuplicateFile),
        ("id", "ONE", Check::TextureDuplicateId),
    ] {
        let original =
            std::mem::replace(&mut built.manifest["textures"][1][field], json!(duplicate));
        assert_fault(&built, expected);
        built.manifest["textures"][1][field] = original;
    }
    directory.close()?;
    Ok(())
}

#[test]
fn all_texture_reference_surfaces_detect_missing_ids() -> package_fault::Result<()> {
    let (directory, mut built) = fixture()?;
    for (path, dangling) in [
        (
            "/materials",
            json!([{"textures": [{"texture": "missing"}]}]),
        ),
        (
            "/meshes",
            json!([{"materials": [{"textures": {"normal": "missing"}}]}]),
        ),
        ("/textures/0/ddna_pair", json!({"smoothness": "missing"})),
    ] {
        let (parent, field) = path.rsplit_once('/').ok_or("test field")?;
        let entry = &mut built.manifest.pointer_mut(parent).ok_or("test parent")?[field];
        let original = std::mem::replace(entry, dangling);
        assert_fault(&built, Check::TextureReference);
        built.manifest.pointer_mut(parent).ok_or("test parent")?[field] = original;
    }
    directory.close()?;
    Ok(())
}

fn fixture() -> package_fault::Result<(tempfile::TempDir, Built)> {
    let directory = tempfile::tempdir()?;
    let out = directory.path().to_owned();
    fs::create_dir(out.join("textures"))?;
    for file in ["textures/one.png", "textures/two.png"] {
        fs::write(out.join(file), [])?;
    }
    let manifest = json!({
        "textures": [
            {
                "source": SOURCE,
                "source_rendition": "specular",
                "encoding": "data",
                "id": "one",
                "file": "textures/one.png"
            },
            {
                "source": SOURCE.to_ascii_lowercase(),
                "source_rendition": "data",
                "encoding": "data",
                "id": "two",
                "file": "textures/two.png"
            }
        ],
        "materials": [],
        "meshes": [],
        "issues": []
    });
    Ok((
        directory,
        Built {
            out,
            report: Value::Null,
            manifest,
            elapsed: Duration::ZERO,
        },
    ))
}

fn assert_fault(built: &Built, expected: Check) {
    let error = verify(built).expect_err("the inconsistent package must fail");
    assert_eq!(
        error.to_string(),
        expected.to_string(),
        "the canary reports the failing invariant"
    );
}
