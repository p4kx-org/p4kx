// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use serde_json::Value;

use crate::model_item_origin_checks::{assert_site, inventory_uses};
use crate::support::TestResult;

pub(super) fn assert_compiled_2_tip_sites(reply: &Value, start: u64) -> TestResult {
    for (output, source, span) in [
        ("controller_id", "controller_id", 72..76),
        ("parent", "parent", 80..84),
        ("relative/translation/2", "relative.translation", 108..112),
        ("world/translation/0", "world.translation", 128..132),
        ("name", "name", 145..149),
    ] {
        assert_bone_span(
            reply,
            &format!("items/0/{output}"),
            source,
            2,
            1,
            start + span.start..start + span.end,
        )?;
    }
    assert_site(
        reply,
        "total",
        "cryEngineSkeleton",
        "boneCount",
        &["2", &start.to_string(), &(start + 4).to_string()],
    )?;
    assert_prefix(reply, "items/0/node", 3)?;
    assert_site(
        reply,
        "items/0/node",
        "cryEngineChunkTable",
        "type",
        &["2", "48", "52"],
    )?;
    assert_site(
        reply,
        "items/0/world/translation/0",
        "cryEngineChunkTable",
        "offset",
        &["2", "56", "64"],
    )?;
    Ok(())
}

pub(super) fn assert_bone_span(
    reply: &Value,
    output: &str,
    name: &str,
    chunk: usize,
    bone: usize,
    span: Range<u64>,
) -> TestResult {
    assert_site(
        reply,
        output,
        "cryEngineSkeleton",
        name,
        &[
            &chunk.to_string(),
            &bone.to_string(),
            &span.start.to_string(),
            &span.end.to_string(),
        ],
    )
}

pub(super) fn assert_prefix(reply: &Value, output: &str, count: usize) -> TestResult {
    assert_site(
        reply,
        output,
        "cryEngineChunkTable",
        "skeletonTypePrefix",
        &["16", "20", "16", &count.to_string()],
    )
}

pub(super) fn assert_retained_bone_positions(
    reply: &Value,
    chunk: usize,
    bones: Range<usize>,
) -> TestResult {
    for site in inventory_uses(reply)? {
        let Some(location) = site.get("location") else {
            continue;
        };
        if location.get("namespace").and_then(Value::as_str) != Some("cryEngineSkeleton")
            || location.get("field").and_then(Value::as_str) == Some("boneCount")
        {
            continue;
        }
        let parse = |path| {
            location
                .pointer(path)
                .and_then(Value::as_str)
                .and_then(|ordinal| ordinal.parse::<usize>().ok())
        };
        assert_eq!(
            parse("/indices/0"),
            Some(chunk),
            "only the selected skeleton chunk"
        );
        assert!(
            parse("/indices/1").is_some_and(|ordinal| bones.contains(&ordinal)),
            "only returned bone positions"
        );
    }
    Ok(())
}
