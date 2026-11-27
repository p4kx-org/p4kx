// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::Archive;
use p4kx_media::{ExportOptions, Identity, MediaKind, MediaRecord, Status};
use serde_json::{Value, json};

use super::animation_definition_origins::Origins;
use super::support::{self, TestResult};

#[path = "animation_clip_contributor_fixture.rs"]
mod fixture;

#[path = "animation_clip_contributor_bank.rs"]
mod bank;

#[path = "animation_clip_contributor_scope.rs"]
mod scope;

use self::fixture::{
    CHILD_ADB, ENTITY, EXACT_WILDCARD_CAF, FOLDED_WILDCARD_CAF, INCLUDE_SHARED_CAF, INCLUDED_CAF,
    INCLUDED_PARAMS, PART_ADB, PART_CAF, PART_ONLY_CAF, PART_PARAMS, ROOT_ADB, ROOT_CAF,
    ROOT_PARAMS,
};

const NAMESPACE: &str = "animation-definition-clip-contributor";

#[test]
fn clip_contributors_join_only_their_physical_rig_placement_and_authored_closures() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = fixture::open(scratch.path())?;
    let output = scratch.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Animation, MediaKind::AnimationDefinition],
        animation_entity: Some(ENTITY.into()),
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "animation-definition.jsonl")?;
    let origins = Origins::read(&output.join(&index.artifact_base))?;
    verify_placements(&archive, &rows)?;
    verify_contributors(&archive, &rows, &origins)?;
    let clips = support::records(&output, &index, "animation.jsonl")?;
    scope::verify(&archive, &rows, &clips)
}

fn verify_placements(archive: &Archive, rows: &[MediaRecord]) -> TestResult {
    let receipts = typed(rows, "scope-receipt")
        .map(|(_, row)| row)
        .collect::<Vec<_>>();
    let bindings = typed(rows, "entity-database-binding")
        .map(|(_, row)| row)
        .collect::<Vec<_>>();
    assert_eq!(receipts.len(), 3, "one receipt per physical rig placement");
    assert_eq!(
        bindings.len(),
        3,
        "one ADB binding per physical rig placement"
    );
    for (index, ((receipt, binding), (params, port))) in receipts
        .into_iter()
        .zip(bindings)
        .zip([
            (ROOT_PARAMS, None),
            (PART_PARAMS, Some("left")),
            (PART_PARAMS, Some("right")),
        ])
        .enumerate()
    {
        assert_eq!(
            receipt.subresource.indices,
            [index as u64],
            "receipt ordinal"
        );
        assert_eq!(
            binding.subresource.indices,
            [index as u64],
            "binding ordinal"
        );
        assert_eq!(
            receipt.data.pointer("/payload/outcome/entry_index"),
            Some(&json!(entry_index(archive, params)?.to_string())),
            "the original rig assigns the intended CHRPARAMS"
        );
        let expected_port = port.map(|name| json!(name));
        assert_eq!(
            receipt.data.pointer("/payload/placement_detail/port_path"),
            expected_port.as_ref(),
            "placement ordinal keeps its own physical port route"
        );
        assert_eq!(
            binding.data.pointer("/payload/port_path"),
            expected_port.as_ref(),
            "the database binding points to the same port"
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Alias {
    parameters: &'static str,
    element: u64,
    path: &'static str,
    candidate: Option<u64>,
    case_variant: bool,
}

impl Alias {
    const fn named(parameters: &'static str, element: u64, path: &'static str) -> Self {
        Self {
            parameters,
            element,
            path,
            candidate: None,
            case_variant: false,
        }
    }
}

struct Expected {
    placement: u64,
    database: &'static str,
    clip: u64,
    alias: Alias,
}

fn root_expectations(result: &mut Vec<Expected>) {
    let root = Alias::named(ROOT_PARAMS, 2, ROOT_CAF);
    let included = Alias::named(INCLUDED_PARAMS, 3, INCLUDE_SHARED_CAF);
    for clip in [5, 6] {
        for alias in [root, included] {
            result.push(Expected {
                placement: 0,
                database: ROOT_ADB,
                clip,
                alias,
            });
        }
    }
    result.push(Expected {
        placement: 0,
        database: CHILD_ADB,
        clip: 5,
        alias: Alias::named(INCLUDED_PARAMS, 2, INCLUDED_CAF),
    });
    for alias in [root, included] {
        result.push(Expected {
            placement: 0,
            database: CHILD_ADB,
            clip: 6,
            alias: Alias {
                case_variant: true,
                ..alias
            },
        });
    }
}

fn mount_expectations(placement: u64, result: &mut Vec<Expected>) {
    let aliases = [
        Alias::named(PART_PARAMS, 2, PART_CAF),
        Alias {
            parameters: PART_PARAMS,
            element: 5,
            path: EXACT_WILDCARD_CAF,
            candidate: Some(0),
            case_variant: false,
        },
        Alias {
            parameters: PART_PARAMS,
            element: 5,
            path: FOLDED_WILDCARD_CAF,
            candidate: Some(1),
            case_variant: true,
        },
    ];
    for alias in aliases {
        result.push(Expected {
            placement,
            database: PART_ADB,
            clip: 5,
            alias,
        });
    }
    result.push(Expected {
        placement,
        database: PART_ADB,
        clip: 7,
        alias: Alias::named(PART_PARAMS, 3, PART_ONLY_CAF),
    });
}

impl Expected {
    fn payload(&self, archive: &Archive) -> Result<Value, Box<dyn Error>> {
        let alias = json!({
            "entry_index": entry_index(archive, self.alias.parameters)?.to_string(),
            "element_ordinal": self.alias.element.to_string(),
            "declaration_ordinal": "1",
            "kind": if self.alias.candidate.is_some() { "wildcard" } else { "named-caf" },
            "candidate_ordinal": self.alias.candidate.unwrap_or_default().to_string(),
        });
        Ok(json!({
            "placement_ordinal": self.placement.to_string(),
            "database_binding_ordinal": self.placement.to_string(),
            "clip": {
                "entry_index": entry_index(archive, self.database)?.to_string(),
                "element_ordinal": self.clip.to_string(),
                "declaration_ordinal": "0",
            },
            "alias": alias,
            "target": {
                "kind": "caf",
                "entry_index": entry_index(archive, self.alias.path)?.to_string(),
            },
            "match_basis": if self.alias.case_variant { "ascii-case-variant" } else { "exact" },
        }))
    }
}

fn verify_contributors(archive: &Archive, rows: &[MediaRecord], origins: &Origins) -> TestResult {
    let mut source = Vec::new();
    root_expectations(&mut source);
    for placement in [1, 2] {
        mount_expectations(placement, &mut source);
    }
    let mut expected = source
        .iter()
        .map(|item| item.payload(archive))
        .collect::<Result<Vec<_>, _>>()?;
    let mut sequences = [0_u64; 3];
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for (ordinal, row) in typed(rows, "clip-contributor") {
        verify_row(row, &mut sequences, &validator)?;
        let payload = row.data.get("payload").ok_or("contributor payload")?;
        let index = expected
            .iter()
            .position(|candidate| candidate == payload)
            .ok_or_else(|| format!("unexpected physical clip/alias join {payload}"))?;
        expected.swap_remove(index);
        verify_origins(archive, ordinal, payload, origins)?;
    }
    assert!(
        expected.is_empty(),
        "all authored clip uses and alias candidates occur exactly once per placement: {expected:?}"
    );
    assert_eq!(
        sequences,
        [7, 4, 4],
        "distinct exact and folded matches per placement"
    );
    Ok(())
}

fn verify_row(
    row: &MediaRecord,
    sequences: &mut [u64],
    validator: &jsonschema::Validator,
) -> TestResult {
    assert!(
        matches!(row.source.identity, Identity::Root),
        "cross-document contributor is directory-owned"
    );
    assert_eq!(row.status, Status::Complete, "complete observed match");
    assert_eq!(
        row.subresource.namespace, NAMESPACE,
        "same contributor identity namespace"
    );
    let [placement, sequence] = row.subresource.indices.as_slice() else {
        return Err("contributor identity needs scope placement and sequence".into());
    };
    assert_eq!(
        row.data.pointer("/payload/placement_ordinal"),
        Some(&json!(placement.to_string())),
        "identity and payload name the same placement"
    );
    let expected = sequences
        .get_mut(usize::try_from(*placement)?)
        .ok_or("fixture placement")?;
    assert_eq!(
        *sequence, *expected,
        "deterministic contiguous candidate order per placement"
    );
    *expected += 1;
    validator
        .validate(&serde_json::to_value(row)?)
        .map_err(|error| error.to_string().into())
}

fn verify_origins(
    archive: &Archive,
    ordinal: usize,
    payload: &Value,
    origins: &Origins,
) -> TestResult {
    let entry = |pointer| {
        payload
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or("original coordinate")
            .map(str::to_owned)
    };
    let clip = entry("/clip/entry_index")?;
    let alias = entry("/alias/entry_index")?;
    let child_is_encrypted = clip == entry_index(archive, CHILD_ADB)?.to_string();
    let alias_is_encrypted = alias == entry_index(archive, INCLUDED_PARAMS)?.to_string();
    origins.marker(
        ordinal,
        "/data/payload/clip/declaration_ordinal",
        child_is_encrypted,
    )?;
    origins.marker(
        ordinal,
        "/data/payload/alias/declaration_ordinal",
        alias_is_encrypted,
    )?;
    origins.marker(
        ordinal,
        "/data/payload/match_basis",
        child_is_encrypted || alias_is_encrypted,
    )?;
    Ok(())
}

fn typed<'a>(
    rows: &'a [MediaRecord],
    kind: &'a str,
) -> impl Iterator<Item = (usize, &'a MediaRecord)> {
    rows.iter()
        .enumerate()
        .filter(move |(_, row)| row.data.get("record_type").and_then(Value::as_str) == Some(kind))
}

fn entry_index(archive: &Archive, path: &str) -> Result<usize, Box<dyn Error>> {
    archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .map(p4kx_extraction::Entry::index)
        .ok_or_else(|| format!("original Entry {path}").into())
}
