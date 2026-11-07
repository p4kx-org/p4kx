// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::claim::Claim;
use crate::installer::fault::Fault;
use crate::installer::journal_codec;
use crate::installer::journal_encode;
use crate::installer::raw_path::{RawPath, StringKind};
use crate::installer::rollback_receipt::RollbackSource;
use crate::installer::state::{Phase, Request, Snapshot};
use crate::installer::tests::{self, ENTRY, PRODUCT, TestResult};

fn rolled_back_install(from: RollbackSource) -> Result<Snapshot, Fault> {
    let mut store = tests::memory(RawPath::Absent);
    let id = tests::begin(
        &mut store,
        Request::Install {
            product: PRODUCT,
            lease: tests::lease()?,
        },
    )?;
    if from != RollbackSource::Prepared {
        tests::apply(&mut store, id)?;
    }
    if from == RollbackSource::Committed {
        tests::commit(&mut store, id)?;
    }
    tests::rollback(&mut store, id)?;
    Ok(store.state)
}

#[test]
fn every_rollback_origin_produces_a_canonical_complete_receipt() -> TestResult {
    for from in [
        RollbackSource::Prepared,
        RollbackSource::Applied,
        RollbackSource::Committed,
    ] {
        let snapshot = rolled_back_install(from)?;
        let receipt = snapshot
            .journal
            .as_ref()
            .and_then(|journal| journal.rollback.as_ref())
            .ok_or("rollback omitted its durable input")?;
        assert_eq!(receipt.from, from, "the actual source phase is retained");
        let encoded = journal_codec::encode(&snapshot)?;
        let decoded = journal_codec::decode(&encoded)?;
        assert_eq!(decoded, snapshot, "durable compensation retains its input");
        assert_eq!(
            journal_codec::encode(&decoded)?,
            encoded,
            "the receipt has one canonical binary encoding"
        );
        for end in 0..encoded.len() {
            assert!(
                journal_codec::decode(encoded.get(..end).ok_or("bad truncation")?).is_err(),
                "no truncated rollback receipt grants an owner"
            );
        }
    }
    Ok(())
}

#[test]
fn legacy_versions_and_unknown_receipt_tags_fail_closed() -> TestResult {
    let encoded = journal_codec::encode(&rolled_back_install(RollbackSource::Prepared)?)?;
    assert!(
        encoded.ends_with(&[1, 0, 0]),
        "the canonical suffix is receipt-present, Prepared, absent observed PATH"
    );
    let mut legacy = encoded.clone();
    *legacy
        .get_mut(journal_codec::MAGIC.len() - 1)
        .ok_or("missing version")? = 1;
    assert_eq!(
        journal_codec::decode(&legacy),
        Err(Fault::CorruptState),
        "a version-one owner cannot bypass the receipt contract"
    );
    for offset in [1, 2, 3] {
        let mut unknown = encoded.clone();
        *unknown
            .get_mut(encoded.len() - offset)
            .ok_or("missing receipt tag")? = 255;
        assert_eq!(
            journal_codec::decode(&unknown),
            Err(Fault::CorruptState),
            "unknown receipt, origin and raw PATH tags are rejected"
        );
    }
    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        journal_codec::decode(&trailing),
        Err(Fault::CorruptState),
        "another value cannot follow the receipt"
    );
    Ok(())
}

#[test]
fn a_receipt_is_required_exactly_for_the_rolled_back_phase() -> TestResult {
    let mut rolled_back = rolled_back_install(RollbackSource::Prepared)?;
    let receipt = rolled_back
        .journal
        .as_mut()
        .ok_or("missing journal")?
        .rollback
        .take()
        .ok_or("missing receipt")?;
    assert_corrupt(&rolled_back)?;
    for phase in [Phase::Prepared, Phase::Applied, Phase::Committed] {
        let mut snapshot = tests::installation_at(RawPath::Absent, phase)?;
        snapshot.journal.as_mut().ok_or("missing journal")?.rollback = Some(receipt.clone());
        assert_corrupt(&snapshot)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum RecoveredClaim {
    Borrowed,
    Inserted,
}

fn recovered(claim: RecoveredClaim) -> Result<Snapshot, Fault> {
    let mut store = tests::memory(tests::path("A", StringKind::Plain));
    tests::install(&mut store)?;
    let id = tests::begin(&mut store, Request::Remove { product: PRODUCT })?;
    tests::apply(&mut store, id)?;
    store.state.path = RawPath::Present {
        kind: StringKind::Expandable,
        bytes: tests::bytes(&match claim {
            RecoveredClaim::Borrowed => format!("F;{ENTRY};G"),
            RecoveredClaim::Inserted => "F;G".into(),
        }),
    };
    tests::rollback(&mut store, id)?;
    journal_codec::decode(&journal_codec::encode(&store.state)?)
}

fn assert_corrupt(snapshot: &Snapshot) -> TestResult {
    assert_eq!(
        journal_codec::encode(snapshot),
        Err(Fault::CorruptState),
        "an inconsistent ownership receipt cannot be persisted"
    );
    assert_eq!(
        journal_codec::decode(&journal_encode::snapshot(snapshot)?),
        Err(Fault::CorruptState),
        "an inconsistent ownership receipt cannot be recovered"
    );
    Ok(())
}

#[test]
fn changing_the_rollback_input_cannot_forge_a_different_reacquired_claim() -> TestResult {
    let mut snapshot = recovered(RecoveredClaim::Inserted)?;
    let owner = snapshot.owner.as_ref().ok_or("missing recovered owner")?;
    let Claim::Inserted(proof) = &owner.claim else {
        return Err("compensation did not reacquire its missing lease".into());
    };
    assert_eq!(
        proof.before,
        RawPath::Present {
            kind: StringKind::Expandable,
            bytes: tests::bytes("F;G"),
        },
        "compensation owns only the insertion against the actual foreign value"
    );
    snapshot
        .journal
        .as_mut()
        .and_then(|journal| journal.rollback.as_mut())
        .ok_or("missing receipt")?
        .observed_path = tests::path("UNRELATED", StringKind::Plain);
    assert_corrupt(&snapshot)
}

#[test]
fn foreign_edits_after_durable_rollback_survive_the_next_remove() -> TestResult {
    for claim in [RecoveredClaim::Borrowed, RecoveredClaim::Inserted] {
        let mut store = tests::memory(RawPath::Absent);
        store.state = recovered(claim)?;
        let foreign = RawPath::Present {
            kind: StringKind::Plain,
            bytes: tests::bytes(&format!("Z;{ENTRY};F;;T")),
        };
        store.state.path = foreign.clone();
        store.state = journal_codec::decode(&journal_codec::encode(&store.state)?)?;
        tests::remove(&mut store, PRODUCT)?;
        let expected = match claim {
            RecoveredClaim::Borrowed => foreign,
            RecoveredClaim::Inserted => RawPath::Present {
                kind: StringKind::Plain,
                bytes: tests::bytes("Z;F;;T"),
            },
        };
        assert_eq!(
            store.state.path, expected,
            "foreign bytes remain authoritative"
        );
    }
    Ok(())
}

#[test]
fn ensure_after_durable_rollback_reacquires_against_the_current_foreign_path() -> TestResult {
    for claim in [RecoveredClaim::Borrowed, RecoveredClaim::Inserted] {
        let mut store = tests::memory(RawPath::Absent);
        store.state = recovered(claim)?;
        let foreign = RawPath::Present {
            kind: StringKind::Plain,
            bytes: tests::bytes("Z;;F"),
        };
        store.state.path = foreign.clone();
        store.state = journal_codec::decode(&journal_codec::encode(&store.state)?)?;
        let id = tests::begin(&mut store, Request::Repair { product: PRODUCT })?;
        tests::apply(&mut store, id)?;
        tests::commit(&mut store, id)?;
        assert_eq!(
            store.state.path,
            RawPath::Present {
                kind: StringKind::Plain,
                bytes: tests::bytes(&format!("{ENTRY};Z;;F")),
            },
            "Ensure uses the current value's exact bytes and string kind"
        );
        tests::remove(&mut store, PRODUCT)?;
        assert_eq!(
            store.state.path, foreign,
            "the new insertion alone is removed"
        );
    }
    Ok(())
}
