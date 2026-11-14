// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, Filter, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, Identity, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestContainer, TestEntry, TestHircObject};
use serde_json::{Value, json};

use crate::support;
use crate::support::audio_provider::{Origins, TestResult};

const ROOT_BANK: &str = "Data/Sounds/wwise/German/root.bnk";
const NESTED_BANK: &str = r"Data\Sounds\wwise\French\nested.bnk";
const GERMAN: &str = "Data/Sounds/wwise/German/00042.wem";
const NEUTRAL: &str = "Data/Sounds/wwise/Media/42.wem";
const CONTAINER: &str = "other.socpak";
const RESOLUTION: &str = "audio-bank-hirc-sound-loose-wem-resolution";
const CANDIDATE: &str = "audio-bank-hirc-sound-loose-wem-candidate";
const INVENTORY: &str = "audio-loose-wem-candidate";
const ZIP_END_BYTES: usize = 22;

#[test]
fn authored_streamed_sound_links_all_physical_candidates_inside_its_own_archive() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = fixture(folder.path())?;
    let output = folder.path().join("media");
    let mut options = options()?;
    options.limits.reference_candidates = 1;
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "audio.jsonl")?;
    let root = BankScope {
        path: ROOT_BANK,
        container: None,
    };
    let nested = BankScope {
        path: NESTED_BANK,
        container: Some(CONTAINER),
    };
    check_summary(&rows, root, &[1, 0], 42, "ambiguous", 3)?;
    check_summary(&rows, root, &[1, 1], 42, "ambiguous", 3)?;
    check_summary(&rows, root, &[1, 2], 92, "missing", 0)?;
    check_summary(&rows, nested, &[1, 0], 42, "resolved", 1)?;
    assert!(
        rows.iter()
            .all(|row| row.subresource.namespace != RESOLUTION
                || !root.owns(row)
                || matches!(row.subresource.indices.as_slice(), [1, 0..=2])),
        "embedded-only and external sources do not receive authored loose references"
    );
    let origins = Origins::read(&output.join(&index.artifact_base))?;
    check_root_candidates(&rows, root, &origins)?;
    let (_, target) = linked(&rows, nested, &[1, 0, 0], 1)?;
    assert_eq!(
        target.data.pointer("/payload/placement/folder"),
        Some(&json!("French")),
        "the nested link joins only the nested inventory"
    );
    validate(&rows)
}

#[test]
fn recovered_streamed_sound_scope_cannot_prove_absence_or_uniqueness() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let bank = TestBank::new(7)
        .object(TestHircObject::sound(1, 42, 2, 0))
        .object(TestHircObject::sound(2, 92, 2, 0))
        .bytes();
    let mut bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored(ROOT_BANK, bank))
        .entry(TestEntry::stored(NEUTRAL, b"unread candidate"))
        .build();
    bytes.truncate(bytes.len().saturating_sub(ZIP_END_BYTES));
    fs::write(&path, bytes)?;
    let archive = Archive::open_with(path, OpenOptions::new().recover(true))?;
    let output = folder.path().join("media");
    let options = ExportOptions {
        diagnostic: true,
        ..options()?
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("a recovered owning directory produced a complete generation".into());
    };
    let directory = diagnostic.parent().ok_or("diagnostic generation")?;
    let rows = support::records(directory, &index, "audio.jsonl")?;
    let root = BankScope {
        path: ROOT_BANK,
        container: None,
    };
    check_summary(&rows, root, &[1, 0], 42, "unavailable", 1)?;
    check_summary(&rows, root, &[1, 1], 92, "unavailable", 0)?;
    linked(&rows, root, &[1, 0, 0], 1)?;
    validate(&rows)
}

fn fixture(directory: &Path) -> TestResult<Archive> {
    let path = directory.join("Data.p4k");
    let root = TestBank::new(7)
        .object(TestHircObject::language_sound(1, 42, 2, 0))
        .object(TestHircObject::language_sound(2, 42, 1, 12))
        .object(TestHircObject::sound(3, 92, 2, 0))
        .object(TestHircObject::sound(4, 42, 0, 12))
        .object(TestHircObject::external_sound(5, 42))
        .bytes();
    let nested_bank = TestBank::new(8)
        .object(TestHircObject::language_sound(1, 42, 2, 0))
        .bytes();
    let nested = TestContainer::new()
        .entry(TestEntry::stored(NESTED_BANK, nested_bank))
        .entry(TestEntry::stored(
            "Data/Sounds/wwise/French/42.wem",
            b"unread nested candidate",
        ))
        .entry(TestEntry::stored(
            "Data/Sounds/wwise/French/92.wem",
            b"same ID as root's missing candidate",
        ))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(ROOT_BANK, root))
        .entry(TestEntry::stored(GERMAN, b"first German candidate"))
        .entry(TestEntry::stored(GERMAN, b"second German candidate").encrypted())
        .entry(TestEntry::stored(NEUTRAL, b"neutral candidate"))
        .entry(TestEntry::stored(CONTAINER, nested))
        .write_to(&path)?;
    Ok(Archive::open(path)?)
}

fn options() -> TestResult<ExportOptions> {
    Ok(ExportOptions {
        kinds: vec![MediaKind::Audio],
        include: Some(Filter::parse("*.bnk")?),
        ..ExportOptions::default()
    })
}

#[derive(Clone, Copy)]
struct BankScope {
    path: &'static str,
    container: Option<&'static str>,
}

impl BankScope {
    fn owns(self, row: &MediaRecord) -> bool {
        match (&row.source.identity, self.container) {
            (Identity::Entry { entry, containers }, None) => {
                entry.path == self.path && containers.is_empty()
            }
            (Identity::Entry { entry, containers }, Some(scope)) => {
                entry.path == self.path
                    && matches!(containers.as_slice(), [owner] if owner.path == scope)
            }
            _ => false,
        }
    }

    fn owns_inventory(self, row: &MediaRecord) -> bool {
        match (&row.source.identity, self.container) {
            (Identity::Root, None) => true,
            (Identity::Entry { entry, containers }, Some(scope)) => {
                entry.path == scope && containers.is_empty()
            }
            _ => false,
        }
    }
}

fn check_summary(
    rows: &[MediaRecord],
    owner: BankScope,
    indices: &[u64],
    media_id: u32,
    status: &str,
    count: usize,
) -> TestResult {
    let (_, summary) = bank_row(rows, owner, RESOLUTION, indices)?;
    let reason = if status == "unavailable" {
        json!({"type":"incomplete-inventory"})
    } else {
        Value::Null
    };
    assert_eq!(
        summary.data,
        json!({
            "record_type":"bank-hirc-sound-loose-wem-resolution",
            "payload":{
                "media_id":media_id, "resolution":status,
                "observed_candidates":count.to_string(), "candidates_complete":reason.is_null(),
                "unavailable_reason":reason
            }
        }),
        "the Sound edge accounts for the exact owning inventory"
    );
    Ok(())
}

fn linked<'rows>(
    rows: &'rows [MediaRecord],
    owner: BankScope,
    indices: &[u64],
    target_index: u64,
) -> TestResult<(usize, &'rows MediaRecord)> {
    let (ordinal, link) = bank_row(rows, owner, CANDIDATE, indices)?;
    assert_eq!(
        link.data,
        json!({
            "record_type":"bank-hirc-sound-loose-wem-candidate",
            "payload":{"target_entry_index":target_index.to_string()}
        }),
        "the candidate carries the original directory Entry key"
    );
    let target = rows
        .iter()
        .find(|row| {
            row.subresource.namespace == INVENTORY
                && row.subresource.indices == [target_index]
                && row.source.root.hex == link.source.root.hex
                && owner.owns_inventory(row)
        })
        .ok_or("candidate joins the canonical owning directory observation")?;
    Ok((ordinal, target))
}

fn bank_row<'rows>(
    rows: &'rows [MediaRecord],
    owner: BankScope,
    namespace: &str,
    indices: &[u64],
) -> TestResult<(usize, &'rows MediaRecord)> {
    rows.iter()
        .enumerate()
        .find(|(_, row)| {
            row.subresource.namespace == namespace
                && row.subresource.indices == indices
                && owner.owns(row)
        })
        .ok_or_else(|| format!("missing {namespace} at {indices:?} in {}", owner.path).into())
}

fn check_root_candidates(rows: &[MediaRecord], owner: BankScope, origins: &Origins) -> TestResult {
    for (candidate, entry) in [1, 2, 3].into_iter().enumerate() {
        let candidate = u64::try_from(candidate)?;
        let (ordinal, target) = linked(rows, owner, &[1, 0, candidate], entry)?;
        linked(rows, owner, &[1, 1, candidate], entry)?;
        assert_eq!(
            target.data.pointer("/payload/selection/status"),
            Some(&json!("not-selected")),
            "the canonical target is observed without selecting its payload"
        );
        if entry == 2 {
            check_locked_candidate(origins, ordinal, target)?;
        }
    }
    Ok(())
}

fn check_locked_candidate(origins: &Origins, ordinal: usize, target: &MediaRecord) -> TestResult {
    assert_eq!(
        target.data.pointer("/payload/payload_access"),
        Some(&json!("key-required")),
        "the canonical target records unavailable encrypted payload access"
    );
    let receipt = origins.field(
        "records",
        ordinal,
        "/data/payload/target_entry_index",
        false,
    )?;
    let references = receipt
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("sound link source use references")?;
    let uses = references
        .iter()
        .map(|reference| origins.physical_use(reference))
        .collect::<TestResult<Vec<_>>>()?;
    for field in ["path", "entry_index"] {
        assert!(
            uses.iter().any(|used| {
                used.source.pointer("/path/value") == Some(&json!(GERMAN))
                    && used.source.get("entryIndex") == Some(&json!(2))
                    && used.source.get("storedEncryption") == Some(&json!("encrypted"))
                    && used.observation.get("scope") == Some(&json!("archiveDirectoryMetadata"))
                    && used.observation.pointer("/location/field") == Some(&json!(field))
            }),
            "the link cites the exact target directory {field} without using its payload"
        );
    }
    for field in ["soundSourceId", "soundStorage", "soundPluginId"] {
        assert!(
            uses.iter().any(|used| {
                used.source.pointer("/path/value") == Some(&json!(ROOT_BANK))
                    && used.source.get("entryIndex") == Some(&json!(0))
                    && used.observation.get("scope") == Some(&json!("payload"))
                    && used.observation.pointer("/location/field") == Some(&json!(field))
            }),
            "the link cites the authored bank {field}"
        );
    }
    Ok(())
}

fn validate(rows: &[MediaRecord]) -> TestResult {
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows
        .iter()
        .filter(|row| matches!(row.subresource.namespace.as_str(), RESOLUTION | CANDIDATE))
    {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "actual edge validates"
        );
    }
    Ok(())
}
