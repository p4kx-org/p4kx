// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::Soundbank;
use p4kx_testkit::{
    ArchiveBuilder, TestAtl, TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase,
    test_key, wwise_short_id,
};
use serde_json::{Value, json};

use super::super::txtp_origin_assertions::check_address;
use super::super::{EMBEDDED, Fixture, PLAY, TestResult, pcm};
use super::{FLAGS, SELECTION, SELECTION_FIELD, TXTP_FILE, selection_origin};

#[path = "wwise_txtp_selection_media.rs"]
mod media;
#[path = "wwise_txtp_selection_route.rs"]
mod route;

const EVENT: &str = "Play_Topology";
const TXTP: &str = "Play_Topology.txtp";
const BANK_PATH: &str = "Data/Sounds/wwise/Topology.bnk";
const CONTAINER: u32 = 10;
const FIRST_SOUND: u32 = 20;
const SECOND_SOUND: u32 = 21;
const FIRST_WEM: u32 = 100;
const SECOND_WEM: u32 = 101;
const MISSING: u32 = 999;
const CHILDREN: [u32; 4] = [MISSING, FIRST_SOUND, FIRST_SOUND, SECOND_SOUND];
const RANDOM_HIRC_TYPE: u8 = 5;
const PLAIN_NODE_LEN: usize = 31;
const MODE_OFFSET: usize = PLAIN_NODE_LEN + 4 * size_of::<u16>() + 3 * size_of::<f32>() + 2;
const RANDOM_SEQUENCE: &str = "bnkHircRandomSequence";
const ID_BYTES: usize = size_of::<u32>();

#[test]
fn selection_attests_to_rendered_physical_children_in_plain_and_encrypted_banks() -> TestResult {
    let bank = bank();
    let expected = original_sites(&bank)?;
    for encrypted in [false, true] {
        let fixture = fixture(&bank, encrypted)?;
        let report = export(&fixture, &[])?;
        let file = report.pointer(TXTP_FILE).ok_or("written TXTP file")?;
        let origin = selection_origin(file)?;
        assert_eq!(
            file["approximations"],
            json!([SELECTION]),
            "only the rendered choice"
        );
        assert_eq!(
            file["breaks"], 1,
            "the absent first authored child is a broken link"
        );
        assert_eq!(
            origin["sourceSetComplete"], false,
            "route and media proof is incomplete"
        );
        assert_eq!(
            origin["containsEncryptedSource"],
            if encrypted { json!(true) } else { Value::Null },
            "an encrypted contributor is conclusive even for incomplete evidence",
        );
        let observed = referenced_sites(file, origin)?;
        assert_eq!(
            observed, expected,
            "only used kind, mode, count and rendered child words"
        );
        check_txtp(&fixture.out, file, origin)?;
    }
    Ok(())
}

fn original_sites(bytes: &[u8]) -> Result<BTreeSet<Site<'static>>, Box<dyn Error>> {
    let bank = Soundbank::parse(bytes)?;
    let object = bank
        .objects()
        .iter()
        .find(|object| object.id == CONTAINER)
        .ok_or("container")?;
    let kind = object.byte_range().start;
    let mode = object.id_byte_range().end + MODE_OFFSET;
    let count = mode + 2;
    let first = count + ID_BYTES;
    assert_eq!(
        bytes.get(kind),
        Some(&RANDOM_HIRC_TYPE),
        "original HIRC kind byte"
    );
    assert_eq!(bytes.get(mode), Some(&0), "original Random mode byte");
    assert_eq!(
        bytes.get(count..first),
        Some(u32::try_from(CHILDREN.len())?.to_le_bytes().as_slice()),
        "original declared child count",
    );
    let version = bank.version_byte_range().ok_or("bank version word")?;
    let mut sites = BTreeSet::from([
        Site::word("bnkBkhd", "version", version),
        Site::word("bnkHircObject", "typeCode", kind..kind + 1),
        Site::word(RANDOM_SEQUENCE, "randomSequenceMode", mode..mode + 1),
        Site::word(RANDOM_SEQUENCE, "containerChildCount", count..first),
    ]);
    for (ordinal, id) in CHILDREN.into_iter().enumerate() {
        let start = first + ordinal * ID_BYTES;
        let end = start + ID_BYTES;
        assert_eq!(
            bytes.get(start..end),
            Some(id.to_le_bytes().as_slice()),
            "child word {ordinal}"
        );
        if id != MISSING {
            sites.insert(Site {
                ordinal: Some(ordinal),
                ..Site::word(RANDOM_SEQUENCE, "containerChildId", start..end)
            });
        }
    }
    Ok(sites)
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Site<'a> {
    namespace: &'a str,
    field: &'a str,
    start: usize,
    end: usize,
    ordinal: Option<usize>,
}

impl<'a> Site<'a> {
    const fn word(namespace: &'a str, field: &'a str, bytes: Range<usize>) -> Self {
        Self {
            namespace,
            field,
            start: bytes.start,
            end: bytes.end,
            ordinal: None,
        }
    }

    fn from_use(observed: &'a Value) -> Result<Self, Box<dyn Error>> {
        assert_eq!(observed["role"], "selector", "the word selects the path");
        assert_eq!(
            observed["scope"], "payload",
            "positions address the bank payload"
        );
        let coordinates = observed
            .pointer("/location/indices")
            .and_then(Value::as_array)
            .ok_or("original byte positions")?
            .iter()
            .map(decimal)
            .collect::<Result<Vec<_>, _>>()?;
        let (start, end, ordinal) = match coordinates.as_slice() {
            [start, end] => (*start, *end, None),
            [start, end, ordinal] => (*start, *end, Some(*ordinal)),
            _ => return Err("two byte coordinates and optional child ordinal".into()),
        };
        Ok(Self {
            namespace: observed
                .pointer("/location/namespace")
                .and_then(Value::as_str)
                .ok_or("original namespace")?,
            field: observed
                .pointer("/location/field")
                .and_then(Value::as_str)
                .ok_or("original field")?,
            start,
            end,
            ordinal,
        })
    }
}

fn decimal(value: &Value) -> Result<usize, Box<dyn Error>> {
    Ok(value.as_str().ok_or("decimal coordinate")?.parse()?)
}

fn referenced_sites<'a>(
    file: &'a Value,
    origin: &'a Value,
) -> Result<BTreeSet<Site<'a>>, Box<dyn Error>> {
    let mut sites = BTreeSet::new();
    for observation in selection_uses(file, origin)? {
        let observation = observation?;
        if observation.is_media() || observation.is_route() {
            continue;
        }
        assert_eq!(observation.path()?, BANK_PATH, "the actual owning bank");
        assert!(
            sites.insert(Site::from_use(observation.site)?),
            "no duplicate use of one physical word"
        );
    }
    Ok(sites)
}

struct SelectionUse<'a> {
    site: &'a Value,
    physical: &'a Value,
}

impl<'a> SelectionUse<'a> {
    fn path(&self) -> Result<&'a str, &'static str> {
        self.physical
            .pointer("/path/value")
            .and_then(Value::as_str)
            .ok_or("physical path")
    }

    fn is_media(&self) -> bool {
        matches!(
            self.namespace(),
            Some("wemBytes" | "bnkDidx" | "bnkSection")
        )
    }

    fn is_route(&self) -> bool {
        matches!(self.namespace(), Some("bnkHircEvent" | "bnkHircAction"))
    }

    fn namespace(&self) -> Option<&'a str> {
        self.site
            .pointer("/location/namespace")
            .and_then(Value::as_str)
    }
}

fn selection_uses<'a>(
    file: &'a Value,
    origin: &'a Value,
) -> Result<impl Iterator<Item = Result<SelectionUse<'a>, &'static str>>, &'static str> {
    let all = file
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source use inventory")?;
    let sources = file
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical sources")?;
    let refs = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("contributor references")?;
    Ok(refs.iter().map(|reference| {
        let site = all
            .iter()
            .find(|source_use| source_use.get("id") == Some(reference))
            .ok_or("resolved use reference")?;
        let physical = sources
            .iter()
            .find(|source| source.get("id") == site.get("source"))
            .ok_or("physical entry")?;
        Ok(SelectionUse { site, physical })
    }))
}

fn check_txtp(out: &Path, file: &Value, origin: &Value) -> TestResult {
    let text = check_receipt(out, file, origin)?;
    let commands: Vec<_> = text
        .lines()
        .map(str::trim_start)
        .filter(|line| {
            line.starts_with("group = ")
                || Path::new(line)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"))
        })
        .collect();
    assert_eq!(
        commands,
        [
            "Play_Topology_100.wav",
            "Play_Topology_100.wav",
            "Play_Topology_101.wav",
            "group = -R3>1"
        ],
        "both occurrences of the first surviving child are rendered"
    );
    Ok(())
}

fn check_receipt(out: &Path, file: &Value, origin: &Value) -> Result<String, Box<dyn Error>> {
    let text = fs::read_to_string(out.join(TXTP))?;
    let receipt = file
        .get("originReceipt")
        .and_then(Value::as_str)
        .ok_or("TXTP receipt name")?;
    let receipt = fs::read_to_string(out.join(receipt))?;
    let mut addressed = receipt.lines().map(serde_json::from_str::<Value>);
    let row = addressed
        .find_map(|row| match row {
            Ok(row)
                if row.pointer("/address/field").and_then(Value::as_str)
                    == Some(SELECTION_FIELD) =>
            {
                Some(Ok(row))
            }
            Err(error) => Some(Err(error)),
            Ok(_) => None,
        })
        .ok_or("receipt's addressed choice")??;
    let _span = check_address(&row, text.as_bytes(), SELECTION, origin)?;
    Ok(text)
}

fn export(fixture: &Fixture, extra_flags: &[&str]) -> Result<Value, Box<dyn Error>> {
    let result = p4kx_testkit::p4kx()?
        .arg("--json")
        .arg("--key")
        .arg(test_key().to_hex())
        .args(["wwise", "extract"])
        .arg(&fixture.archive)
        .arg(EVENT)
        .arg("--out")
        .arg(&fixture.out)
        .args(FLAGS)
        .args(extra_flags)
        .output()?;
    assert_eq!(
        result.status.code(),
        Some(0),
        "synthetic export: {result:?}"
    );
    Ok(serde_json::from_slice(&result.stdout)?)
}

fn fixture(bank: &[u8], encrypted: bool) -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let out = dir.path().join("out");
    let definitions = TestAtl::new("SC_Audio").trigger(EVENT, &[EVENT]);
    let bank = if encrypted {
        TestEntry::zstd(BANK_PATH, bank).encrypted()
    } else {
        TestEntry::stored(BANK_PATH, bank)
    };
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data/Libs/GameAudio/SC_Audio.xml",
            definitions.to_cryxml(),
        ))
        .entry(bank)
        .write_to(&archive)?;
    Ok(Fixture {
        _dir: dir,
        archive,
        out,
    })
}

fn bank() -> Vec<u8> {
    TestBank::new(wwise_short_id("Topology"))
        .version(154)
        .wem(FIRST_WEM, pcm())
        .wem(SECOND_WEM, pcm())
        .object(TestHircObject::event(wwise_short_id(EVENT), &[1]))
        .object(TestHircObject::action(1, PLAY, CONTAINER))
        .object(TestHircObject::container(
            TestContainerKind::Random,
            CONTAINER,
            &CHILDREN,
            TestNodeBase::Plain,
        ))
        .object(TestHircObject::sound(FIRST_SOUND, FIRST_WEM, EMBEDDED, 0))
        .object(TestHircObject::sound(SECOND_SOUND, SECOND_WEM, EMBEDDED, 0))
        .bytes()
}
