// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject, wwise_short_id};
use serde_json::{Value, json};

use super::{PLAY, TestResult};

#[path = "wwise_resolve_media_origin_tests.rs"]
mod media;

const HOME: &str = "Data\\Sounds\\wwise\\Home.bnk";
const TARGET: &str = "Data\\Sounds\\wwise\\Target.bnk";
const DECOY: &str = "Data\\Sounds\\wwise\\Decoy.bnk";

struct Banks {
    home: Vec<u8>,
    target: Vec<u8>,
}

fn fixture(path: &Path) -> Result<Banks, Box<dyn Error>> {
    let atl = TestAtl::new("SC_PathOrigins").trigger("Probe", &["Actual_Event", "Absent_Event"]);
    let home = TestBank::new(wwise_short_id("Home"))
        .object(TestHircObject::event(
            wwise_short_id("Actual_Event"),
            &[1, 2],
        ))
        .object(TestHircObject::action_in_bank(
            1,
            PLAY,
            10,
            wwise_short_id("Target"),
        ))
        .object(TestHircObject::action_in_bank(
            2,
            PLAY,
            10,
            wwise_short_id("MissingBank"),
        ))
        .bytes();
    let target = TestBank::new(wwise_short_id("Target"))
        .object(TestHircObject::sound(10, 5000, 2, 0))
        .bytes();
    let decoy = TestBank::new(wwise_short_id("Decoy"))
        .object(TestHircObject::sound(10, 6000, 2, 0))
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_PathOrigins.xml",
            atl.to_cryxml(),
        ))
        .entry(TestEntry::stored(HOME, home.clone()))
        .entry(TestEntry::zstd(TARGET, target.clone()).encrypted())
        .entry(TestEntry::stored(DECOY, decoy))
        .write_to(path)?;
    Ok(Banks { home, target })
}

#[test]
fn wwise_path_origins_hold_exact_object_bytes_across_banks() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let banks = fixture(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let result = probe(&path, "Probe")?;
    let proof = Proof {
        result: &result,
        archive: &archive,
    };
    for (index, (bank, bytes, encrypted, id)) in [
        (HOME, &banks.home, false, wwise_short_id("Actual_Event")),
        (HOME, &banks.home, false, 1),
        (TARGET, &banks.target, true, 10),
    ]
    .into_iter()
    .enumerate()
    {
        proof.step(
            &format!("/variants/0/path/{index}"),
            bank,
            bytes,
            encrypted,
            id,
        )?;
    }
    proof.check_breaks(&banks.home)?;
    assert_eq!(
        proof.origin("/variants/0/sound_id")?,
        proof.origin("/variants/0/path/2/id")?,
        "sound_id aliases the exact last object ID",
    );
    Ok(())
}

fn probe(path: &Path, trigger: &str) -> Result<Value, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .env("P4KX_ARCHIVE_KEY", p4kx_testkit::test_key().to_hex())
        .args(["--json", "wwise", "resolve"])
        .arg(path)
        .arg(trigger)
        .output()?;
    assert!(output.status.success(), "the fixture resolves: {output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

struct Proof<'a> {
    result: &'a Value,
    archive: &'a Archive,
}

impl Proof<'_> {
    fn step(&self, path: &str, bank: &str, bytes: &[u8], encrypted: bool, id: u32) -> TestResult {
        let byte_range = object_bytes(bytes, id)?;
        let id_offset = byte_range.start + 5;
        let kind = self.origin(&format!("{path}/kind"))?;
        let object_id = self.origin(&format!("{path}/id"))?;
        for field in [kind, object_id] {
            assert_eq!(
                field["containsEncryptedSource"], encrypted,
                "{path} inherits its own bank encryption"
            );
            assert_eq!(
                field["sourceSetComplete"], true,
                "{path} has complete byte evidence"
            );
        }
        assert_eq!(
            self.result.pointer(&format!("{path}/id")),
            Some(&json!(id)),
            "{path} retains the original object ID"
        );
        assert_eq!(
            kind["sourceUsesRefs"].as_array().map(Vec::len),
            Some(2),
            "kind uses the object and version"
        );
        assert_eq!(
            object_id["sourceUsesRefs"].as_array().map(Vec::len),
            Some(1),
            "ID comes from its own bytes alone"
        );
        self.site(
            object_id,
            bank,
            "bnkHircObject",
            "id",
            id_offset..id_offset + 4,
        )?;
        self.site(kind, bank, "bnkHircObject", "objectExtent", byte_range)?;
        self.site(kind, bank, "bnkBkhd", "version", 8..12)?;
        Ok(())
    }

    fn origin(&self, path: &str) -> Result<&Value, Box<dyn Error>> {
        self.result["fieldOrigins"]
            .get(path)
            .ok_or_else(|| "immediate path marker absent".into())
    }

    fn site(
        &self,
        field: &Value,
        bank: &str,
        namespace: &str,
        name: &str,
        span: Range<usize>,
    ) -> TestResult {
        let indices = [span.start.to_string(), span.end.to_string()];
        self.site_at(field, bank, namespace, name, &indices)?;
        Ok(())
    }

    fn site_at(
        &self,
        field: &Value,
        bank: &str,
        namespace: &str,
        name: &str,
        indices: &[String],
    ) -> Result<&Value, Box<dyn Error>> {
        let uses = self
            .result
            .pointer("/provenance/uses")
            .and_then(Value::as_array)
            .ok_or("uses")?;
        let refs = field["sourceUsesRefs"].as_array().ok_or("use references")?;
        let site = uses
            .iter()
            .find(|row| {
                refs.contains(&row["id"])
                    && row.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                    && row.pointer("/location/field").and_then(Value::as_str) == Some(name)
            })
            .ok_or("selected use")?;
        assert_eq!(
            site["scope"], "payload",
            "the byte span is in the decoded bank payload"
        );
        assert_eq!(
            site.pointer("/location/indices"),
            Some(&json!(indices)),
            "bank-relative byte bounds are exact",
        );
        let sources = self
            .result
            .pointer("/provenance/sources")
            .and_then(Value::as_array)
            .ok_or("sources")?;
        let owner = sources
            .iter()
            .find(|source| source["id"] == site["source"])
            .ok_or("physical source")?;
        let index = self
            .archive
            .entries()
            .iter()
            .position(|entry| entry.path() == bank)
            .ok_or("original bank entry index")?;
        assert_eq!(
            owner["entryIndex"].as_u64(),
            Some(u64::try_from(index)?),
            "equal short IDs retain their physical bank identity"
        );
        Ok(site)
    }

    fn check_breaks(&self, home: &[u8]) -> TestResult {
        let breaks = self.result["breaks"].as_array().ok_or("breaks")?;
        let other_bank = breaks
            .iter()
            .position(|value| value["kind"] == "other_soundbank")
            .ok_or("failed foreign bank reference")?;
        let no_bank = breaks
            .iter()
            .position(|value| value["kind"] == "no_soundbank")
            .ok_or("synthetic event step")?;
        self.step(
            &format!("/breaks/{other_bank}/path/1"),
            HOME,
            home,
            false,
            2,
        )?;
        for field in ["kind", "id"] {
            assert_eq!(
                self.origin(&format!("/breaks/{other_bank}/object/{field}"))?,
                self.origin(&format!("/breaks/{other_bank}/path/1/{field}"))?,
                "the break object aliases the retained real prefix",
            );
            for address in ["path/0", "object"] {
                let unknown = self.origin(&format!("/breaks/{no_bank}/{address}/{field}"))?;
                assert_eq!(
                    unknown,
                    &json!({
                        "containsEncryptedSource": null,
                        "sourceUsesRefs": [],
                        "sourceSetComplete": false,
                    }),
                    "a synthetic event without a bank has no HIRC bytes",
                );
            }
        }
        Ok(())
    }
}

fn object_bytes(bank: &[u8], id: u32) -> Result<Range<usize>, Box<dyn Error>> {
    let hirc = bank
        .windows(4)
        .position(|window| window == b"HIRC")
        .ok_or("HIRC")?;
    let first = hirc + 8;
    let mut object = first + 4;
    for _ in 0..word(bank, first)? {
        let len = usize::try_from(word(bank, object + 1)?)?;
        let end = object + 5 + len;
        if word(bank, object + 5)? == id {
            return Ok(object..end);
        }
        object = end;
    }
    Err("fixture object is absent".into())
}

fn word(bank: &[u8], start: usize) -> Result<u32, Box<dyn Error>> {
    Ok(u32::from_le_bytes(
        bank.get(start..start + 4)
            .ok_or("fixture word")?
            .try_into()?,
    ))
}
