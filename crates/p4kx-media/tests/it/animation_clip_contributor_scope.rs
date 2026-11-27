// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_media::{Identity, MediaRecord};
use p4kx_testkit::TestSkeleton;
use serde_json::{Value, json};

use super::super::support::TestResult;
use super::fixture::{PART_ADB, PART_CAF, ROOT_ADB, ROOT_CAF};

struct Case {
    placement: u64,
    database: &'static str,
    clip: &'static str,
    weight: &'static str,
    rate: u16,
    end_frame: u32,
}

pub(super) fn verify(
    archive: &Archive,
    definitions: &[MediaRecord],
    clips: &[MediaRecord],
) -> TestResult {
    for case in [
        Case {
            placement: 0,
            database: ROOT_ADB,
            clip: ROOT_CAF,
            weight: "0.25",
            rate: 24,
            end_frame: 12,
        },
        Case {
            placement: 1,
            database: PART_ADB,
            clip: PART_CAF,
            weight: "0.75",
            rate: 60,
            end_frame: 48,
        },
        Case {
            placement: 2,
            database: PART_ADB,
            clip: PART_CAF,
            weight: "0.75",
            rate: 60,
            end_frame: 48,
        },
    ] {
        let database = super::entry_index(archive, case.database)?.to_string();
        let target = super::entry_index(archive, case.clip)?.to_string();
        verify_fragment_scope(definitions, &case, &database, &target)?;
        verify_clip_scope(clips, &case)?;
    }
    Ok(())
}

fn verify_fragment_scope(
    rows: &[MediaRecord],
    case: &Case,
    database: &str,
    target: &str,
) -> TestResult {
    let placement = case.placement.to_string();
    let contributor = rows
        .iter()
        .find(|row| {
            row.subresource.namespace == "animation-definition-clip-contributor"
                && text(row, "/payload/placement_ordinal") == Some(placement.as_str())
                && text(row, "/payload/target/entry_index") == Some(target)
                && text(row, "/payload/clip/element_ordinal") == Some("5")
        })
        .ok_or("addressable contributor for the authored fragment")?;
    assert_eq!(
        text(contributor, "/payload/clip/entry_index"),
        Some(database),
        "contributor database belongs to the physical placement"
    );
    assert_eq!(
        text(contributor, "/payload/database_binding_ordinal"),
        Some(placement.as_str()),
        "contributor keeps its own ADB binding"
    );

    let declaration = rows
        .iter()
        .find(|row| {
            at(row, "animation-definition-clip-reference", case.database)
                && row.subresource.indices == [5, 0]
        })
        .ok_or("original ADB clip declaration")?;
    assert_eq!(
        text(declaration, "/payload/authored_name"),
        Some("shared"),
        "both rigs use the same alias name"
    );
    assert_eq!(
        text(declaration, "/payload/owner/element_ordinal"),
        Some("3"),
        "the declaration addresses its original fragment"
    );

    assert!(
        rows.iter().any(|row| {
            at(row, "animation-definition-graph-attribute", case.database)
                && row.subresource.indices.first() == Some(&3)
                && text(row, "/payload/field") == Some("option-weight")
                && text(row, "/payload/declaration/value") == Some(case.weight)
        }),
        "the fragment retains the option authored for this rig"
    );
    Ok(())
}

fn verify_clip_scope(rows: &[MediaRecord], case: &Case) -> TestResult {
    let rig = rows
        .iter()
        .find(|row| {
            row.subresource.namespace == "animation-entity-rig"
                && row.subresource.indices == [case.placement]
        })
        .ok_or("independent physical rig")?;
    assert_eq!(
        text(rig, "/payload/skeleton/status"),
        Some("read"),
        "both physical rigs have readable bones"
    );

    let source = rows
        .iter()
        .find(|row| {
            row.subresource.namespace == "animation-entity-source"
                && text(row, "/payload/path") == Some(case.clip)
        })
        .ok_or("physical entity clip source")?;
    let source_ordinal = source
        .subresource
        .indices
        .first()
        .ok_or("entity source ordinal")?;
    assert!(
        rows.iter().any(|row| {
            row.subresource.namespace == "animation-entity-source-link"
                && row.subresource.indices.first() == Some(&case.placement)
                && text(row, "/payload/disposition") == Some("named-caf")
                && row.data.pointer("/payload/source").and_then(Value::as_u64)
                    == Some(*source_ordinal)
        }),
        "the exact named CAF belongs to its own rig placement"
    );
    verify_clip_records(rows, case, *source_ordinal)
}

fn verify_clip_records(rows: &[MediaRecord], case: &Case, source_ordinal: u64) -> TestResult {
    let entity_clip = rows
        .iter()
        .find(|row| {
            row.subresource.namespace == "animation-entity-clip"
                && row.subresource.indices == [source_ordinal, 0]
        })
        .ok_or("independent entity clip")?;
    assert_eq!(
        entity_clip.data.pointer("/payload/frames_per_second"),
        Some(&json!(case.rate)),
        "same-name clips retain their original rates"
    );
    assert_eq!(
        entity_clip.data.pointer("/payload/end_frame"),
        Some(&json!(case.end_frame)),
        "same-name clips retain their original frame spans"
    );

    let clip = rows
        .iter()
        .find(|row| at(row, "animation-clip", case.clip))
        .ok_or("physical clip metadata")?;
    assert_eq!(
        entity_clip.data.pointer("/payload/physical_record/source"),
        Some(&serde_json::to_value(&clip.source.identity)?),
        "entity clip references the original physical CAF"
    );
    let controller = rows
        .iter()
        .find(|row| at(row, "animation-controller", case.clip))
        .ok_or("physical clip controller")?;
    assert_eq!(
        controller.data.pointer("/payload/controller_id"),
        Some(&json!(TestSkeleton::controller_id(0))),
        "the shared controller ID is preserved on each physical source"
    );
    Ok(())
}

fn at(row: &MediaRecord, namespace: &str, path: &str) -> bool {
    row.subresource.namespace == namespace
        && matches!(&row.source.identity, Identity::Entry { entry, .. } if entry.path == path)
}

fn text<'row>(row: &'row MediaRecord, pointer: &str) -> Option<&'row str> {
    row.data.pointer(pointer).and_then(Value::as_str)
}
