// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::claim::Claim;
use crate::installer::fault::Fault;
use crate::installer::journal_codec;
use crate::installer::raw_path::{RawPath, StringKind};
use crate::installer::state::{Phase, Snapshot};
use crate::installer::tests;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn original() -> RawPath {
    RawPath::Present {
        kind: StringKind::Expandable,
        bytes: vec![b';', 0, b'A', 0, 0, 0, 0, 0, 0x00, 0xd8],
    }
}

#[test]
fn every_durable_phase_round_trips_exact_raw_bytes_and_owner_proof() -> TestResult {
    for phase in [
        Phase::Prepared,
        Phase::Applied,
        Phase::Committed,
        Phase::RolledBack,
    ] {
        let snapshot = tests::installation_at(original(), phase)?;
        let encoded = journal_codec::encode(&snapshot)?;
        assert_eq!(
            journal_codec::decode(&encoded)?,
            snapshot,
            "the durable phase and unpaired UTF16/tail bytes survive encoding"
        );
    }
    Ok(())
}

#[test]
fn all_truncated_inputs_and_trailing_data_are_rejected() -> TestResult {
    let encoded = journal_codec::encode(&tests::installed(original())?)?;
    for size in 0..encoded.len() {
        let truncated = encoded.get(..size).ok_or("invalid fixture truncation")?;
        assert!(
            journal_codec::decode(truncated).is_err(),
            "a partially durable record is never an empty owner"
        );
    }
    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        journal_codec::decode(&trailing),
        Err(Fault::CorruptState),
        "trailing values cannot conceal another receipt"
    );
    Ok(())
}

#[test]
fn unknown_version_and_value_tags_are_rejected() -> TestResult {
    let mut encoded = journal_codec::encode(&Snapshot {
        reservation: None,
        cancelled_binding: None,
        path: RawPath::Absent,
        owner: None,
        journal: None,
        sequence: 0,
    })?;
    let version = encoded
        .get_mut(journal_codec::MAGIC.len() - 1)
        .ok_or("missing format version")?;
    *version = 255;
    assert_eq!(
        journal_codec::decode(&encoded),
        Err(Fault::CorruptState),
        "future version cannot be misread as current ownership"
    );
    *encoded
        .get_mut(journal_codec::MAGIC.len() - 1)
        .ok_or("missing version")? = *journal_codec::MAGIC
        .last()
        .ok_or("missing current version")?;
    let tag_offset = journal_codec::MAGIC.len() + size_of::<u64>();
    *encoded.get_mut(tag_offset).ok_or("missing path tag")? = 255;
    assert_eq!(
        journal_codec::decode(&encoded),
        Err(Fault::CorruptState),
        "unknown path kind cannot become an absent value"
    );
    Ok(())
}

#[test]
fn hostile_length_prefix_is_bounded_before_copying_a_blob() {
    let mut bytes = journal_codec::MAGIC.to_vec();
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        journal_codec::decode(&bytes),
        Err(Fault::StateLimit),
        "the declared length is rejected before payload allocation"
    );
}

#[test]
fn outer_size_bound_is_checked_before_parsing() {
    let oversized = vec![0; journal_codec::MAX_STATE_BYTES + 1];
    assert_eq!(
        journal_codec::decode(&oversized),
        Err(Fault::StateLimit),
        "unbounded Registry state is not parsed"
    );
}

#[test]
fn inconsistent_identity_and_fabricated_insertion_proof_cannot_be_encoded() -> TestResult {
    let mut snapshot = tests::installed(original())?;
    snapshot.sequence += 1;
    assert_eq!(
        journal_codec::encode(&snapshot),
        Err(Fault::CorruptState),
        "sequence must match the journal receipt"
    );
    snapshot.sequence -= 1;
    let owner = snapshot.owner.as_mut().ok_or("missing owner fixture")?;
    let Claim::Inserted(insertion) = &mut owner.claim else {
        return Err("fixture claim was not inserted".into());
    };
    insertion.after = RawPath::Absent;
    assert_eq!(
        journal_codec::encode(&snapshot),
        Err(Fault::CorruptState),
        "ownership requires the exact canonical splice proof"
    );
    Ok(())
}
