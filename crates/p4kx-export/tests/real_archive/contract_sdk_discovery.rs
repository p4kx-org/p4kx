// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{ArchiveWatch, DataForge, RecordTree, TreeBuilder};
use serde::{Deserialize, Serialize};

use super::source::Rows;
use super::{TestResult, check};

const PREFIX: &str = "libs/foundry/records/contracts/contractgenerator/";

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Discovery {
    inspected_headers: usize,
    candidate_bodies: usize,
    unreadable_headers: usize,
    unreadable_bodies: usize,
    malformed_source_shapes: usize,
    unmatched_source_features: usize,
}

pub(super) fn first<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<(RecordTree<'source>, Discovery)> {
    let mut counts = Discovery::default();
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        check(watch)?;
        counts.inspected_headers += 1;
        match record.type_name() {
            Ok("ContractGenerator") => {}
            Ok(_) => continue,
            Err(_) => {
                counts.unreadable_headers += 1;
                continue;
            }
        }
        let Ok(path) = record.record_path() else {
            counts.unreadable_headers += 1;
            continue;
        };
        if !matches_prefix(path) {
            continue;
        }
        counts.candidate_bodies += 1;
        let Ok(tree) = trees.build(&record) else {
            counts.unreadable_bodies += 1;
            continue;
        };
        check(watch)?;
        let Ok(rows) = Rows::read(&tree) else {
            counts.malformed_source_shapes += 1;
            continue;
        };
        if rows.has_required_source_features() {
            return Ok((tree, counts));
        }
        counts.unmatched_source_features += 1;
    }
    Err("no original Generator exercises the required parent, sub-contract, property-set and Record-reference features".into())
}

fn matches_prefix(path: &str) -> bool {
    path.get(..PREFIX.len()).is_some_and(|candidate| {
        candidate.bytes().zip(PREFIX.bytes()).all(|(left, right)| {
            if left == b'\\' { b'/' } else { left }.eq_ignore_ascii_case(&right)
        })
    })
}
