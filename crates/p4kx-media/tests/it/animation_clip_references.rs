// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_media::{ExportOptions, Identity, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::animation_definition_origins::Origins;
use super::support::{self, TestResult};

const DATABASE: &str = "Data/Animations/clips.adb";
const SPACE: &str = "Data/Animations/clips.bspace";
const COMBINED: &str = "Data/Animations/clips.comb";
const ADB: &str = concat!(
    r#"<AnimDB xmlns:x="urn:fixture"><FragmentList><Idle>"#,
    r#"<Fragment Tags="Ready" OptionWeight="0.25"><AnimLayer>"#,
    r#"<Animation speed="1" name="same"/><Animation name="same"/>"#,
    r#"<Animation x:name="shadow"/><x:Animation name="foreign"/><Animation/>"#,
    r#"</AnimLayer><ProcLayer><Animation name="procedural"/></ProcLayer></Fragment>"#,
    r#"<Fragment Tags="Ready" OptionWeight="0.75"><AnimLayer>"#,
    r#"<Animation name="same"/><Animation name=""/>"#,
    r#"</AnimLayer></Fragment></Idle></FragmentList>"#,
    r#"<FragmentBlendList><Blend from="Ready" to="Next"><OptionA>"#,
    r#"<Fragment TransitionTags="Next"><AnimLayer><Animation name="transition"/></AnimLayer></Fragment>"#,
    r#"</OptionA></Blend></FragmentBlendList></AnimDB>"#,
);
const BSPACE: &str = concat!(
    r#"<ParaGroup xmlns:x="urn:fixture"><ExampleList>"#,
    r#"<Example Alias="repeat" AName="same"/><Example AName="same"/><Example AName=""/>"#,
    r#"<Example x:AName="shadow"/><Example AName=" loose&amp;clip "/>"#,
    r#"</ExampleList><ExamplePseudo><Pseudo Alias="same" AName="not-a-clip"/></ExamplePseudo>"#,
    r#"<Blendable><Face p0="same"/></Blendable></ParaGroup>"#,
);
const COMB: &[u8] = br#"<CombinedBlendSpace><BlendSpaces>
  <BlendSpace AName="Animations/clips.bspace"/>
</BlendSpaces></CombinedBlendSpace>"#;

struct Expected {
    indices: [u64; 2],
    name: &'static str,
    lexeme: &'static str,
}

const ADB_SELECTORS: [Expected; 5] = [
    Expected {
        indices: [5, 1],
        name: "same",
        lexeme: r#"name="same""#,
    },
    Expected {
        indices: [6, 0],
        name: "same",
        lexeme: r#"name="same""#,
    },
    Expected {
        indices: [14, 0],
        name: "same",
        lexeme: r#"name="same""#,
    },
    Expected {
        indices: [15, 0],
        name: "",
        lexeme: r#"name="""#,
    },
    Expected {
        indices: [21, 0],
        name: "transition",
        lexeme: r#"name="transition""#,
    },
];
const SPACE_SELECTORS: [Expected; 4] = [
    Expected {
        indices: [2, 1],
        name: "same",
        lexeme: r#"AName="same""#,
    },
    Expected {
        indices: [3, 0],
        name: "same",
        lexeme: r#"AName="same""#,
    },
    Expected {
        indices: [4, 0],
        name: "",
        lexeme: r#"AName="""#,
    },
    Expected {
        indices: [6, 0],
        name: " loose&clip ",
        lexeme: r#"AName=" loose&amp;clip ""#,
    },
];

#[test]
fn clip_references_preserve_authored_sites_and_remain_unresolved_without_owning_scope() -> TestResult
{
    let scratch = tempfile::tempdir()?;
    let archive = fixture(scratch.path())?;
    let output = scratch.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "animation-definition.jsonl")?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    let origins = Origins::read(&output.join(index.artifact_base))?;
    let database = clips(&rows, DATABASE);
    let space = clips(&rows, SPACE);
    verify_selectors(&database, &ADB_SELECTORS, ADB, &origins, true)?;
    verify_selectors(&space, &SPACE_SELECTORS, BSPACE, &origins, false)?;
    verify_fragment_owners(&database);
    verify_example_owners(&space);
    verify_owner_origins(&rows, &database, &origins, true)?;
    verify_owner_origins(&rows, &space, &origins, false)?;
    assert!(
        clips(&rows, COMBINED).is_empty(),
        "a combined blend-space input remains a file link"
    );
    Ok(())
}

fn fixture(folder: &Path) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = folder.join("clips.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(DATABASE, ADB.as_bytes()).encrypted())
        .entry(TestEntry::stored(SPACE, BSPACE.as_bytes()))
        .entry(TestEntry::stored(COMBINED, COMB))
        .entry(TestEntry::stored(
            "Data/Animations/same.caf",
            b"unselected CAF",
        ))
        .write_to(&path)?;
    Ok(Archive::open_with_key(path, Some(test_key()))?)
}

struct Clip<'rows> {
    ordinal: usize,
    row: &'rows MediaRecord,
}

fn clips<'rows>(rows: &'rows [MediaRecord], path: &str) -> Vec<Clip<'rows>> {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| {
            row.data.get("record_type").and_then(Value::as_str) == Some("clip-reference")
                && matches!(&row.source.identity, Identity::Entry { entry, .. } if entry.path == path)
        })
        .map(|(ordinal, row)| Clip { ordinal, row })
        .collect()
}

fn verify_selectors(
    clips: &[Clip<'_>],
    expected: &[Expected],
    xml: &str,
    origins: &Origins,
    encrypted: bool,
) -> TestResult {
    assert_eq!(clips.len(), expected.len(), "one row per exact selector");
    let mut previous_end = 0;
    for (clip, expected) in clips.iter().zip(expected) {
        let row = clip.row;
        assert_eq!(row.status, Status::Complete, "complete declaration");
        assert_eq!(
            row.subresource.namespace, "animation-definition-clip-reference",
            "each selector has the clip reference namespace"
        );
        assert_eq!(
            row.subresource.indices, expected.indices,
            "original source element and declaration ordinals"
        );
        assert_eq!(
            row.data.pointer("/payload/authored_name"),
            Some(&json!(expected.name)),
            "original name for selector {:?}",
            expected.indices
        );
        let reason = if expected.name.is_empty() {
            "empty-selector"
        } else {
            "no-owning-scope"
        };
        assert_eq!(
            row.data.pointer("/payload/resolution"),
            Some(&json!({"state": "unresolved", "reason": reason})),
            "standalone matching archive names cannot supply a clip target"
        );
        let position = row
            .data
            .pointer("/payload/selector_position")
            .ok_or("selector position")?;
        let start = decimal(position, "start")?;
        let end = decimal(position, "end_exclusive")?;
        assert_eq!(
            position.get("type"),
            Some(&json!("text")),
            "plain XML selector uses a byte span"
        );
        assert!(start >= previous_end, "original declarations stay in order");
        assert_eq!(
            xml.get(start..end),
            Some(expected.lexeme),
            "original XML attribute bytes"
        );
        previous_end = end;
        verify_selector_origin(clip, origins, encrypted, start, end)?;
    }
    Ok(())
}

fn decimal(position: &Value, field: &str) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(position
        .get(field)
        .and_then(Value::as_str)
        .ok_or("position coordinate")?
        .parse()?)
}

fn verify_owner_origins(
    records: &[MediaRecord],
    clips: &[Clip<'_>],
    origins: &Origins,
    encrypted: bool,
) -> TestResult {
    for clip in clips {
        let Identity::Entry { entry, .. } = &clip.row.source.identity else {
            return Err("clip owner physical source Entry".into());
        };
        for field in [
            "owner/element_ordinal",
            "owner_parent/element_ordinal",
            "layer_ordinal",
        ] {
            let Some(index) = clip.row.data.pointer(&format!("/payload/{field}")) else {
                continue;
            };
            let index: u64 = index.as_str().ok_or("owner element ordinal")?.parse()?;
            let position = records
                .iter()
                .find(|row| {
                    row.data.get("record_type").and_then(Value::as_str) == Some("graph-node")
                        && row.subresource.indices == [index]
                        && matches!(&row.source.identity, Identity::Entry { entry: node, .. } if node.entry_index == entry.entry_index)
                })
                .and_then(|row| row.data.pointer("/payload/position"))
                .ok_or("original owner graph node position")?;
            let proof = origins.proof(
                clip.ordinal,
                &format!("/data/payload/{field}"),
                entry.entry_index,
                encrypted,
            )?;
            assert_eq!(
                proof.pointer("/location/indices"),
                Some(&json!([position["start"], position["end_exclusive"]])),
                "owner provenance identifies the owner element"
            );
        }
    }
    Ok(())
}

fn verify_selector_origin(
    clip: &Clip<'_>,
    origins: &Origins,
    encrypted: bool,
    start: usize,
    end: usize,
) -> TestResult {
    let Identity::Entry { entry, .. } = &clip.row.source.identity else {
        return Err("clip declaration physical source Entry".into());
    };
    for field in [
        "/data/payload/authored_name",
        "/data/payload/selector_position",
    ] {
        let use_ = origins.proof(clip.ordinal, field, entry.entry_index, encrypted)?;
        assert_eq!(
            use_.get("location"),
            Some(&json!({
                "type": "subresource",
                "namespace": "animation-definition-xml",
                "indices": [start.to_string(), end.to_string()],
                "field": "declaration",
            })),
            "per-value proof names those exact source bytes"
        );
    }
    Ok(())
}

fn verify_fragment_owners(clips: &[Clip<'_>]) {
    let fragments = [3_u64, 3, 12, 12, 19];
    let layers = [4_u64, 4, 13, 13, 20];
    let parents = [
        ("fragment-group", 2_u64),
        ("fragment-group", 2),
        ("fragment-group", 2),
        ("fragment-group", 2),
        ("transition-option", 18),
    ];
    for (((clip, fragment), layer), (kind, parent)) in
        clips.iter().zip(fragments).zip(layers).zip(parents)
    {
        let data = &clip.row.data;
        assert_eq!(
            data.pointer("/payload/owner"),
            Some(&json!({"kind": "mannequin-fragment", "element_ordinal": fragment.to_string()})),
            "each clip belongs to its own fragment alternative"
        );
        assert_eq!(
            data.pointer("/payload/owner_parent"),
            Some(&json!({"kind": kind, "element_ordinal": parent.to_string()})),
            "fragment parent is the original group or transition option"
        );
        assert_eq!(
            data.pointer("/payload/layer_ordinal"),
            Some(&json!(layer.to_string())),
            "clip belongs to the layer enclosing its declaration"
        );
    }
}

fn verify_example_owners(clips: &[Clip<'_>]) {
    for (clip, example) in clips.iter().zip([2_u64, 3, 4, 6]) {
        let data = &clip.row.data;
        assert_eq!(
            data.pointer("/payload/owner"),
            Some(&json!({"kind": "blend-space-example", "element_ordinal": example.to_string()})),
            "each example retains its own source position"
        );
        assert!(
            data.pointer("/payload/owner_parent").is_none(),
            "blend-space examples omit the Mannequin parent"
        );
        assert!(
            data.pointer("/payload/layer_ordinal").is_none(),
            "blend-space examples omit the Mannequin layer"
        );
    }
}

pub(super) fn verify_scoped(rows: &[MediaRecord]) -> TestResult {
    let records = clips(rows, "Data/Animations/Scope/attachment.adb");
    let [clip] = records.as_slice() else {
        return Err("shared attached ADB has a single authored clip declaration".into());
    };
    assert_eq!(
        clip.row.data.pointer("/payload/authored_name"),
        Some(&json!("attachmentClip")),
        "selected attachment contributes its own original clip name"
    );
    assert_eq!(
        clip.row.data.pointer("/payload/resolution"),
        Some(&json!({"state": "unresolved", "reason": "source-join-unavailable"})),
        "an admitted ADB has owning context without a proven clip target"
    );
    Ok(())
}
