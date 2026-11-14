// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use crate::audio_candidate_fixture as fixture;
use crate::support::{Server, TestResult};

pub(crate) fn info(server: &mut Server, arguments: &Value, path: &str, count: usize) -> TestResult {
    let info = server.ok("get_wem_info", arguments)?;
    assert_eq!(
        info.get("locations_total"),
        Some(&json!(count)),
        "all actual indexed candidate locations are counted"
    );
    assert_eq!(
        info.pointer("/described/path"),
        Some(&json!(path)),
        "the first genuinely readable source path wins"
    );
    assert_eq!(
        info.pointer("/wem/codec"),
        Some(&json!("pcm")),
        "the late complete original candidate is actually parsed"
    );
    assert_eq!(
        info.pointer("/wem/prefetch"),
        Some(&json!(false)),
        "a complete candidate is not reported as a prefetched prefix"
    );
    assert_eq!(
        info.pointer("/wem/sample_count"),
        Some(&json!(8)),
        "the original exact frame count survives"
    );
    Ok(())
}

pub(crate) fn excerpt(server: &mut Server, arguments: &Value, path: &str) -> TestResult {
    let result = server.call("get_audio_excerpt", arguments)?;
    assert_ne!(
        result.get("isError"),
        Some(&json!(true)),
        "a real readable candidate must not be reported as prefetch-only/error: {result}"
    );
    let meta = result
        .get("structuredContent")
        .ok_or("actual excerpt metadata")?;
    metadata(meta, path);
    audio(&result)
}

fn metadata(meta: &Value, path: &str) {
    assert_eq!(
        meta.pointer("/source/path"),
        Some(&json!(path)),
        "the concrete actually decoded source is retained"
    );
    assert_eq!(
        meta.get("codec"),
        Some(&json!("pcm")),
        "actual PCM decoding"
    );
    assert_eq!(
        meta.get("frames"),
        Some(&json!(8)),
        "exact actual original excerpt frames"
    );
    assert_eq!(
        meta.get("truncated"),
        Some(&json!(false)),
        "the exact eight requested frames fit"
    );
}

fn audio(result: &Value) -> TestResult {
    let audio = result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|blocks| {
            blocks
                .iter()
                .find(|block| block.get("type").and_then(Value::as_str) == Some("audio"))
        })
        .ok_or("actual audio content block")?;
    assert_eq!(
        audio.get("mimeType"),
        Some(&json!("audio/wav")),
        "existing audio response contract"
    );
    let wav = STANDARD.decode(
        audio
            .get("data")
            .and_then(Value::as_str)
            .ok_or("actual base64 WAV")?,
    )?;
    assert!(wav.starts_with(b"RIFF"), "the actual result is a RIFF WAV");
    assert!(
        wav.ends_with(&fixture::pcm()),
        "the decoded original PCM words match the independent literal sample oracle"
    );
    Ok(())
}

pub(crate) fn arguments() -> Value {
    json!({"wem_id":fixture::ID,"duration_seconds":0.001})
}

pub(crate) fn fallbacks(server: &mut Server, original: &fixture::Original) -> TestResult {
    let info = server.ok("get_wem_info", &json!({"wem_id":fixture::ID}))?;
    assert_eq!(
        info.get("locations_total"),
        Some(&json!(9)),
        "the known full candidate inventory is retained"
    );
    assert_eq!(
        info.pointer("/described/path"),
        Some(&json!(fixture::bank_path(1))),
        "first observed fallback metadata remains stable"
    );
    assert_eq!(
        info.pointer("/wem/prefetch"),
        Some(&json!(true)),
        "the first original fallback is genuinely prefetched"
    );
    let error = server.error("get_audio_excerpt", &arguments())?;
    if let Some(failure) = &original.first_error {
        assert_eq!(
            failure.path,
            fixture::bank_path(2),
            "original first real error position"
        );
        assert!(
            error.contains(&failure.path) && error.contains(&failure.reason),
            "the first actual read/decode error retains priority: {error}"
        );
    } else {
        assert!(
            error.contains("prefetched start"),
            "only actual all-prefetch candidates justify PrefetchOnly: {error}"
        );
    }
    Ok(())
}
