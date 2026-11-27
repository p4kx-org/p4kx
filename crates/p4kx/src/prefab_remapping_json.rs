// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ResolvedPrefab;
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct InstanceJson<'a> {
    instance_path: &'a [usize],
    target: Option<TargetJson<'a>>,
    blocks: Vec<Vec<PairJson<'a>>>,
}

#[derive(Serialize)]
struct TargetJson<'a> {
    file: &'a str,
    prefab_index: usize,
}

#[derive(Serialize)]
struct PairJson<'a> {
    default_guid: Option<&'a str>,
    instance_guid: Option<&'a str>,
}

pub(super) fn rows(resolved: &ResolvedPrefab) -> Vec<InstanceJson<'_>> {
    resolved
        .guid_remapping_instances()
        .iter()
        .map(|occurrence| InstanceJson {
            instance_path: occurrence.instance_path(),
            target: occurrence.entered_target().map(|target| TargetJson {
                file: target.file.file_name(),
                prefab_index: target.prefab_index,
            }),
            blocks: occurrence
                .blocks()
                .iter()
                .map(|block| {
                    block
                        .iter()
                        .map(|pair| PairJson {
                            default_guid: pair.default_guid.as_deref(),
                            instance_guid: pair.instance_guid.as_deref(),
                        })
                        .collect()
                })
                .collect(),
        })
        .collect()
}
