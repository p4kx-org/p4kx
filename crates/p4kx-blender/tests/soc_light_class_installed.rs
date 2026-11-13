// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Installed archive witness for authored SOC light inheritance.

use p4kx_assembly::{SocContainerCapture, SocMemberSelection};
use p4kx_blender::SocScene;
use p4kx_extraction::{Archive, ArchiveWatch, CancelToken, Filter};
use p4kx_model::SocEntityFile;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

type Check<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_direct_light_class_defaults_reach_the_authored_export() -> Check {
    verify().map_err(|_| "installed authored SOC class witness failed".into())
}

fn verify() -> Check {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let cancel = CancelToken::new();
    let scope = Filter::new("Data/ObjectContainers/Ships/*.socpak");
    let mut entries: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| scope.matches(entry.path()))
        .collect();
    entries.sort_by_key(|entry| entry.uncompressed_size());
    for entry in entries {
        watch.poll()?;
        let Ok(container) = archive.open_container(entry) else {
            continue;
        };
        let capture = SocContainerCapture::read(&[&archive, &container], &cancel)?;
        if !contains_direct_lights(&capture) {
            continue;
        }
        let scene =
            serde_json::to_value(SocScene::from_archive_capture(&capture, &archive, &cancel)?)?;
        if has_inherited_field(&scene) {
            watch.check()?;
            return Ok(());
        }
    }
    Err("no inherited direct light field in the selected source scope".into())
}

fn contains_direct_lights(capture: &SocContainerCapture) -> bool {
    capture
        .occurrences()
        .iter()
        .flat_map(|occurrence| &occurrence.streams)
        .filter_map(|stream| {
            stream
                .member
                .as_ref()
                .and_then(SocMemberSelection::selected)
        })
        .filter_map(|member| member.payload().ok())
        .filter_map(|bytes| SocEntityFile::parse(bytes).ok())
        .any(|file| {
            file.entity_chunks().flatten().any(|chunk| {
                chunk
                    .entities()
                    .any(|entity| entity.authored_lights().next().is_some())
            })
        })
}

fn has_inherited_field(scene: &Value) -> bool {
    rows(scene.get("occurrences"))
        .flat_map(|row| rows(row.get("streams")))
        .flat_map(|row| rows(row.pointer("/member/data")))
        .flat_map(|row| rows(row.get("entities")))
        .flat_map(|row| rows(row.get("lights")))
        .flat_map(|row| rows(row.pointer("/effective/fields")))
        .any(|field| {
            field.get("selection").and_then(Value::as_str) == Some("inherited")
                && field.pointer("/value/state").and_then(Value::as_str) == Some("present")
                && field
                    .pointer("/value/sourceSetComplete")
                    .and_then(Value::as_bool)
                    == Some(true)
                && field
                    .pointer("/value/sourceUsesRefs")
                    .and_then(Value::as_array)
                    .is_some_and(|uses| uses.len() > 2)
        })
}

fn rows(value: Option<&Value>) -> impl Iterator<Item = &Value> {
    value.and_then(Value::as_array).into_iter().flatten()
}
