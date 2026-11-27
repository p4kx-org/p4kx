// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Write, stdout};
use std::time::SystemTime;

use p4kx_extraction::{Archive, ArchiveWatch, Guid};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde::Serialize;

use crate::CatalogOptions;
use crate::archive_key_use::ArchiveKeyUse;
use crate::build;
use crate::build::original_input::test_support::{self, TestResult};
use crate::contract::Source;
use crate::starmap_containers::jump_class::JumpEndpointRecords;

#[path = "starmap_jump_survey_graph.rs"]
mod graph;

#[path = "starmap_jump_survey_records.rs"]
mod records;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Summary<'archive> {
    source: Source<'archive>,
    directory_recovered: bool,
    endpoint_records_complete: bool,
    graph: graph::GraphSurvey,
    records: records::RecordSurvey,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_live_jump_relation_survey() -> TestResult {
    let path = real_archive_path()?;
    if !p4kx_channels::channel_name(&path).is_some_and(|name| name.eq_ignore_ascii_case("LIVE")) {
        return Err("the jump relation survey requires an installed LIVE Archive".into());
    }
    let archive = Archive::open_with_key(path, Some(real_archive_key()?))
        .map_err(|_| "unable to open installed LIVE Archive")?;
    let Some(build) = archive.build() else {
        return Err("installed LIVE Build identity is unavailable".into());
    };
    let parts = [
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    ];
    if parts.iter().any(|field| field.trim().is_empty()) || archive.file_modified().is_none() {
        return Err("installed LIVE Build identity or timestamp is incomplete".into());
    }
    let watch = ArchiveWatch::new(&archive);
    let started = SystemTime::now();
    let mut report = None;
    test_support::with_archive_input(&archive, |forge, input| {
        let options = CatalogOptions::new(started);
        let endpoints = JumpEndpointRecords::read(forge, input, &options)?;
        let graph = graph::survey(&archive, &endpoints)?;
        let records = records::survey(forge, input, &endpoints)?;
        report = Some(Summary {
            source: build::source(&archive, started, None, ArchiveKeyUse::of(&archive)),
            directory_recovered: crate::export_archive_health::inventory(&archive).is_some(),
            endpoint_records_complete: endpoints.complete,
            graph,
            records,
        });
        Ok(())
    })
    .map_err(|_| "jump relation source inspection failed")?;
    watch
        .check()
        .map_err(|_| "installed LIVE input changed during the jump relation survey")?;
    let report = report.ok_or("jump relation survey produced no receipt")?;
    let mut out = stdout().lock();
    serde_json::to_writer(&mut out, &report)?;
    writeln!(out)?;
    if report.graph.roots == 0 || report.graph.reachable.endpoint_declarations == 0 {
        return Err("survey needs original system roots and endpoint declarations".into());
    }
    Ok(())
}

fn endpoint_text(text: &str, endpoints: &JumpEndpointRecords) -> bool {
    text.trim()
        .parse::<Guid>()
        .is_ok_and(|guid| endpoints.get(guid).is_some())
}

fn relation_field(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "partner",
        "target",
        "destination",
        "connection",
        "endpoint",
        "tunnel",
        "exit",
        "link",
        "portal",
    ]
    .into_iter()
    .any(|part| name.contains(part))
}
