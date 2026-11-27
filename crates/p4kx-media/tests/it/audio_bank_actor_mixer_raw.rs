// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::HircLayout;
use p4kx_testkit::{TestBank, TestHircObject, TestNodeBase};
use serde_json::json;

use super::audio_bank_state_provider::{archive, check_retained, options, row, verify_layout};
use super::support;
use super::support::audio_provider::TestResult;

#[test]
fn actor_mixer_keeps_its_original_bank_and_physical_object_in_media() -> TestResult {
    let directory = tempfile::tempdir()?;
    let source = TestBank::new(3)
        .object(TestHircObject::actor_mixer(
            86,
            &[37, 19, 37],
            TestNodeBase::Full,
        ))
        .bytes();
    verify_layout(&source, HircLayout::Decoded)?;
    let archive = archive(directory.path(), &source)?;
    let output = directory.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let records = support::records(&output, &index, "audio.jsonl")?;
    let (_, object) = row(&records, "audio-bank-object", &[1, 0])?;
    assert_eq!(
        object.data.pointer("/payload/object_id"),
        Some(&json!(86)),
        "exact physical ActorMixer object identity"
    );
    assert_eq!(
        object.data.pointer("/payload/kind"),
        Some(&json!("actor-mixer")),
        "authored type 7 in the proved version"
    );
    check_retained(&output.join(&index.artifact_base), object, &source)
}
