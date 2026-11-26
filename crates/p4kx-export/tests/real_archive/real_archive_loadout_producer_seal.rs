// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use p4kx_export::LoadoutSummary;
use p4kx_extraction::Checksum;
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use super::TestResult;

const MAX_METADATA_BYTES: u64 = 1 << 20;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProducerSeal {
    status: String,
    producer_revision: String,
    frame_hash_verified_after: bool,
    input_seals: Inputs,
    result: ReplayResult,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Inputs {
    producer_receipt: FileSeal,
    producer_stdout: FileSeal,
}

#[derive(Deserialize)]
struct FileSeal {
    bytes: u64,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplayResult {
    decoded_sha256: String,
}

impl ProducerSeal {
    pub(crate) fn read() -> TestResult<Self> {
        let path = env::var_os("P4KX_CATALOG_REPLAY_RECEIPT")
            .map(PathBuf::from)
            .ok_or("configure the completed Catalog replay receipt")?;
        let mut file = File::open(path)?.take(MAX_METADATA_BYTES + 1);
        let mut input = Vec::new();
        file.read_to_end(&mut input)?;
        if u64::try_from(input.len())? > MAX_METADATA_BYTES {
            return Err("Catalog replay receipt exceeds the metadata bound".into());
        }
        let replay: Self = serde_json::from_slice(&input)?;
        if replay.status != "complete" || !replay.frame_hash_verified_after {
            return Err("the replay receipt must attest a completed, sealed Catalog frame".into());
        }
        Ok(replay)
    }

    pub(crate) fn producer_revision(&self) -> &str {
        &self.producer_revision
    }

    pub(super) fn producer_stdout(&self, directory: &Path) -> TestResult<String> {
        self.input_seals
            .producer_receipt
            .read(&directory.join("receipt.private.json"))?;
        let bytes = self
            .input_seals
            .producer_stdout
            .read(&directory.join("producer.stdout.log"))?;
        Ok(String::from_utf8(bytes)?)
    }

    pub(super) fn check_frame(&self, decoded_digest: [u8; 32]) -> TestResult {
        if !self
            .result
            .decoded_sha256
            .eq_ignore_ascii_case(&Checksum::Sha256(decoded_digest).hex())
        {
            return Err("the retained Catalog does not match the sealed decoded bytes".into());
        }
        Ok(())
    }
}

impl FileSeal {
    fn read(&self, path: &Path) -> TestResult<Vec<u8>> {
        if self.bytes == 0 || self.bytes > MAX_METADATA_BYTES {
            return Err("Catalog producer metadata has an invalid sealed length".into());
        }
        let mut file = File::open(path)?.take(self.bytes + 1);
        let mut input = Vec::new();
        file.read_to_end(&mut input)?;
        let digest = Checksum::Sha256(Sha256::digest(&input).into());
        if u64::try_from(input.len())? != self.bytes
            || !self.sha256.eq_ignore_ascii_case(&digest.hex())
        {
            return Err("Catalog producer metadata differs from its replay seal".into());
        }
        Ok(input)
    }
}

pub(super) fn check_counts(log: &str, counts: LoadoutSummary) -> TestResult {
    let observed = format!(
        "loadouts: {} ships and {} items, {} ports, {} hold an Entity ({} docked vehicles), {} unresolved, {} Entities without an entry in items or ships, {} loadouts with issues",
        counts.ships,
        counts.items,
        counts.ports,
        counts.attached,
        counts.docked_vehicles,
        counts.unresolved,
        counts.outside,
        counts.with_issues
    );
    if summary_line(log, " ships and ")? != observed {
        return Err(format!(
            "the Loadout counts differ from the sealed producer summary: {observed}"
        )
        .into());
    }
    let written = summary_line(log, " written, ")?
        .strip_prefix("loadouts: ")
        .and_then(|line| line.split_once(" written, "))
        .ok_or("the sealed producer has no Loadout output count")?
        .0
        .parse::<usize>()?;
    if written != counts.ships + counts.items {
        return Err(
            "the Loadout output count differs from the sealed producer kind summary".into(),
        );
    }
    Ok(())
}

fn summary_line<'log>(log: &'log str, marker: &str) -> TestResult<&'log str> {
    let mut lines = log
        .lines()
        .filter(|line| line.starts_with("loadouts: ") && line.contains(marker));
    let line = lines
        .next()
        .ok_or("the sealed transcript has no required Loadout summary")?;
    if lines.next().is_some() {
        return Err("the sealed transcript repeats a Loadout summary".into());
    }
    Ok(line)
}

#[test]
fn producer_census_requires_both_exact_terminal_counts() -> TestResult {
    let kind = "loadouts: 2 written, 0 skipped, complete";
    let details = "loadouts: 1 ships and 1 items, 12 ports, 5 hold an Entity (1 docked vehicles), 3 unresolved, 2 Entities without an entry in items or ships, 1 loadouts with issues";
    let counts = LoadoutSummary {
        ships: 1,
        items: 1,
        ports: 12,
        attached: 5,
        docked_vehicles: 1,
        unresolved: 3,
        outside: 2,
        with_issues: 1,
    };
    let log = format!("{kind}\r\n{details}\r\n");
    check_counts(&log, counts)?;
    assert!(
        check_counts(details, counts).is_err(),
        "the producer kind count is required"
    );
    assert!(
        check_counts(&format!("{log}{details}"), counts).is_err(),
        "duplicate details must fail"
    );
    assert!(
        check_counts(&log.replace("2 Entities", "1 Entities"), counts).is_err(),
        "outside attachments must match exactly"
    );
    assert!(
        check_counts(&log.replace("2 written", "3 written"), counts).is_err(),
        "the written count must match too"
    );
    Ok(())
}
