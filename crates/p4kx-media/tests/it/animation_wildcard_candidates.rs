// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::{Archive, Exclusion, Filter};
use p4kx_media::{ExportOptions, Identity, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestModel, test_key};
use serde_json::{Value, json};

use super::animation_definition_origins::Origins;
use super::support::{self, TestResult};

const MODEL: &str = "Data/Objects/wildcards.chr";
const PARAM_PATH: &str = "Data/Objects/wildcards.chrparams";
const INCLUDED_PARAMS: &str = "Data/Objects/shared.chrparams";
const DECOY_PARAMS: &str = "Data/Objects/unrelated.chrparams";
const SOURCE: &str = "Data/Animations/Fixture/Clips/HighJump_Zed.CAF";
const A: &str = "Data/Animations/Fixture/Clips/A/HighJump_same.caf";
const B: &str = "Data/Animations/Fixture/Clips/B/HighJump_same.caf";
const NESTED: &str = "Data/Animations/Fixture/Clips/A/Nested/HighJump_low.caf";
const EXCLUDED_CLIP: &str = "Data/Animations/Fixture/Clips/X/HighJump_excluded.caf";
const SHARED_CLIP: &str = "Data/Animations/Fixture/Shared/Clips/HighJump_Skill.caf";
const COMB: &str = "Data/Animations/Fixture/Blend/Root.comb";
const CHILD: &str = "Data/Animations/Fixture/Blend/child.bspace";
const DIRECT: &str = "Data/Animations/Fixture/Direct/Skill.bspace";
const EXCLUDED_SPACE: &str = "Data/Animations/Fixture/Blend/excluded.bspace";
const AMBIGUOUS_SPACE: &str = "Data/Animations/Fixture/Blend/ambiguous.bspace";
const AMBIGUOUS_VARIANT: &str = "Data/Animations/Fixture/Blend/AMBIGUOUS.bspace";
const UNRELATED_SPACE: &str = "Data/Animations/Fixture/Unrelated.bspace";
const PARAMS: &str = concat!(
    "<Params><AnimationList>",
    r#"<Animation Name="missing_*" Path="*.caf"/>"#,
    r##"<Animation Name="#FilePath" Path="AniMations\Fixture\"/>"##,
    r#"<Animation Name="local_*" Path="cLiPs\HIGHJUMP_*.caf"/>"#,
    r#"<Animation Name="tree_*" Path="Clips\*\HIGHJUMP_*.caf"/>"#,
    r#"<Animation Name="unsupported_*" Path="Clips/**/*.caf"/>"#,
    r#"<Animation Name="mix_*" Path="Blend/*.comb"/>"#,
    r#"<Animation Name="curve_*" Path="Direct/*.bspace"/>"#,
    r##"<Animation Name="#filepath" Path=""/>"##,
    r#"<Animation Name="reset_*" Path="*.caf"/>"#,
    r##"<Animation Name="#filepath"/>"##,
    r#"<Animation Name="malformed_*" Path="*.caf"/>"#,
    r#"<Animation Name="$Include" Path="Data/Objects/shared.chrparams"/>"#,
    "</AnimationList></Params>",
);
const SHARED: &str = concat!(
    "<Params><AnimationList>",
    r#"<Animation Name="unscoped_*" Path="Clips/*.caf"/>"#,
    r##"<Animation Name="#Filepath" Path="Animations/Fixture/Shared"/>"##,
    r#"<Animation Name="shared_*" Path="Clips/HighJump_*.caf"/>"#,
    "</AnimationList></Params>",
);
const COMBINED: &[u8] = br#"<CombinedBlendSpace><BlendSpaces>
  <BlendSpace AName="aNiMations\Fixture\Blend\CHILD.bspace"/>
  <BlendSpace AName="Data/Animations/Fixture/Blend/child.bspace"/>
  <BlendSpace AName="Animations/Fixture/Blend/excluded.bspace"/>
  <BlendSpace AName="Animations/Fixture/Blend/ambiguous.bspace"/>
  <BlendSpace AName="Animations/Fixture/Blend/missing.bspace"/>
</BlendSpaces></CombinedBlendSpace>"#;
const BSPACE: &[u8] =
    br#"<ParaGroup><ExampleList><Example AName="synthetic"/></ExampleList></ParaGroup>"#;

#[test]
fn wildcard_candidates_preserve_physical_matches_and_scope_reached_blend_spaces() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = fixture(scratch.path())?;
    let output = scratch.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        animation_model: Some(MODEL.into()),
        include: Some(Filter::parse(DECOY_PARAMS)?),
        exclude: Exclusion::parse([EXCLUDED_CLIP, EXCLUDED_SPACE])?,
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
    verify_expansions(&rows);
    verify_matching(&archive, &rows, &origins)?;
    verify_blend_documents(&rows);
    verify_combined_links(&archive, &rows)?;
    verify_include_link(&archive, &rows)
}

fn fixture(folder: &Path) -> Result<Archive, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    let mut archive = ArchiveBuilder::new()
        .entry(TestEntry::stored(MODEL, TestModel::cube().cgf()))
        .entry(TestEntry::stored(PARAM_PATH, PARAMS.as_bytes()).encrypted())
        .entry(TestEntry::stored(INCLUDED_PARAMS, SHARED.as_bytes()))
        .entry(TestEntry::stored(DECOY_PARAMS, b"unselected fixture").with_crc32(0))
        .entry(
            TestEntry::stored(SOURCE, b"unread CAF")
                .encrypted()
                .with_crc32(0),
        )
        .entry(TestEntry::stored(COMB, COMBINED))
        .entry(TestEntry::stored(CHILD, BSPACE).encrypted())
        .entry(TestEntry::stored(DIRECT, BSPACE));
    for path in [
        A,
        B,
        NESTED,
        SHARED_CLIP,
        EXCLUDED_CLIP,
        EXCLUDED_SPACE,
        AMBIGUOUS_SPACE,
        AMBIGUOUS_VARIANT,
        UNRELATED_SPACE,
        "Data/Animations/Fixture/Clips/SmallJump_other.caf",
        "Data/Animations/Fixture/Clips/HighJump_library.dba",
        "Data/Animations/Fixture/ClipsExtra/HighJump_neighbor.caf",
    ] {
        archive = archive.entry(TestEntry::stored(path, b"unread fixture").with_crc32(0));
    }
    archive.write_to(&path)?;
    Ok(Archive::open_with_key(path, Some(test_key()))?)
}

fn verify_expansions(rows: &[MediaRecord]) {
    let mut actual = typed(rows, "wildcard-expansion")
        .map(|(_, row)| {
            assert_eq!(
                row.subresource.namespace, "animation-definition-wildcard-expansion",
                "source expansion keeps the corresponding ordinal domain"
            );
            (
                entry_path(row),
                row.subresource.indices.as_slice(),
                string(row, "/payload/state"),
            )
        })
        .collect::<Vec<_>>();
    actual.sort_unstable();
    let mut expected = [
        (PARAM_PATH, [2, 1].as_slice(), "missing-filepath"),
        (PARAM_PATH, [4, 1].as_slice(), "expanded"),
        (PARAM_PATH, [5, 1].as_slice(), "expanded"),
        (PARAM_PATH, [6, 1].as_slice(), "unsupported-pattern"),
        (PARAM_PATH, [7, 1].as_slice(), "expanded"),
        (PARAM_PATH, [8, 1].as_slice(), "expanded"),
        (PARAM_PATH, [10, 1].as_slice(), "reset-filepath"),
        (PARAM_PATH, [12, 1].as_slice(), "malformed-filepath"),
        (INCLUDED_PARAMS, [2, 1].as_slice(), "missing-filepath"),
        (INCLUDED_PARAMS, [4, 1].as_slice(), "expanded"),
    ];
    expected.sort_unstable();
    assert_eq!(
        actual, expected,
        "each original wildcard retains its own outcome and imports start with empty folder scope"
    );
}

struct Candidate<'a> {
    path: &'a str,
    alias: &'a str,
    family: &'a str,
}

fn verify_matching(archive: &Archive, rows: &[MediaRecord], origins: &Origins) -> TestResult {
    let expected = [
        (4, SOURCE, "local_Zed", "caf"),
        (5, SOURCE, "tree_Zed", "caf"),
        (5, A, "tree_same", "caf"),
        (5, B, "tree_same", "caf"),
        (5, NESTED, "tree_low", "caf"),
        (5, EXCLUDED_CLIP, "tree_excluded", "caf"),
        (7, COMB, "mix_Root", "combined-space"),
        (8, DIRECT, "curve_Skill", "blend-space"),
        (4, SHARED_CLIP, "shared_Skill", "caf"),
    ];
    let candidates = typed(rows, "wildcard-candidate").collect::<Vec<_>>();
    assert_eq!(
        candidates.len(),
        expected.len(),
        "only literal folder matches, including recursively placed alias collisions"
    );
    for (selector, path, alias, family) in expected {
        let (ordinal, row) = candidates
            .iter()
            .copied()
            .find(|(_, row)| {
                let source = if path == SHARED_CLIP {
                    INCLUDED_PARAMS
                } else {
                    PARAM_PATH
                };
                row.subresource.indices.starts_with(&[selector, 1])
                    && string(row, "/payload/archive_path") == path
                    && entry_path(row) == source
            })
            .ok_or_else(|| format!("physical wildcard candidate {path} for selector {selector}"))?;
        verify_candidate(
            archive,
            row,
            &Candidate {
                path,
                alias,
                family,
            },
        )?;
        if path == SOURCE {
            verify_origin(ordinal, origins)?;
        }
    }
    assert_sequences(&candidates);
    Ok(())
}

fn verify_candidate(archive: &Archive, row: &MediaRecord, expected: &Candidate<'_>) -> TestResult {
    assert_eq!(
        row.subresource.namespace, "animation-definition-wildcard-candidate",
        "candidate has a distinct tested namespace"
    );
    assert_eq!(
        row.data.pointer("/payload/entry_index"),
        Some(&json!(entry_index(archive, expected.path)?.to_string())),
        "the physical Entry index addresses the directory's original path"
    );
    assert_eq!(
        string(row, "/payload/alias"),
        expected.alias,
        "alias preserves the captured suffix"
    );
    assert_eq!(
        string(row, "/payload/family"),
        expected.family,
        "directory target family is explicit"
    );
    if expected.path == EXCLUDED_CLIP || expected.path == COMB {
        assert_eq!(
            string(row, "/payload/selection"),
            if expected.path == EXCLUDED_CLIP {
                "excluded"
            } else {
                "included"
            },
            "selection does not erase the original directory observation"
        );
    }
    Ok(())
}

fn assert_sequences(candidates: &[(usize, &MediaRecord)]) {
    for (source, selector) in [
        (PARAM_PATH, 4),
        (PARAM_PATH, 5),
        (PARAM_PATH, 7),
        (PARAM_PATH, 8),
        (INCLUDED_PARAMS, 4),
    ] {
        for (sequence, (_, candidate)) in candidates
            .iter()
            .filter(|(_, row)| {
                entry_path(row) == source && row.subresource.indices.starts_with(&[selector, 1])
            })
            .enumerate()
        {
            assert_eq!(
                candidate.subresource.indices,
                [selector, 1, sequence as u64],
                "candidate order completes the original selector identity"
            );
        }
    }
}

fn verify_origin(ordinal: usize, origins: &Origins) -> TestResult {
    let alias = origins.marker(ordinal, "/data/payload/alias", true)?;
    assert!(
        alias
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .is_some_and(|refs| refs.len() >= 2),
        "derived alias uses the encrypted selector and the directory path"
    );
    let _ = origins.marker(ordinal, "/data/payload/archive_path", false)?;
    let _ = origins.marker(ordinal, "/data/payload/entry_index", false)?;
    Ok(())
}

fn verify_blend_documents(rows: &[MediaRecord]) {
    let mut actual = typed(rows, "document")
        .map(|(_, row)| entry_path(row))
        .collect::<Vec<_>>();
    actual.sort_unstable();
    let mut expected = [CHILD, COMB, DIRECT, PARAM_PATH, INCLUDED_PARAMS];
    expected.sort_unstable();
    assert_eq!(
        actual, expected,
        "scoped documents include the unique referenced child and omit excluded, ambiguous, and unrelated entries"
    );
}

fn verify_combined_links(archive: &Archive, rows: &[MediaRecord]) -> TestResult {
    let links = typed(rows, "link")
        .filter(|(_, row)| entry_path(row) == COMB)
        .collect::<Vec<_>>();
    assert_eq!(
        links.len(),
        5,
        "each combined-space selector stays addressable, including repeated links"
    );
    let child = entry_index(archive, CHILD)?;
    let expected = [
        (r"aNiMations\Fixture\Blend\CHILD.bspace", "resolved"),
        ("Data/Animations/Fixture/Blend/child.bspace", "resolved"),
        ("Animations/Fixture/Blend/excluded.bspace", "not-selected"),
        ("Animations/Fixture/Blend/ambiguous.bspace", "ambiguous"),
        ("Animations/Fixture/Blend/missing.bspace", "missing"),
    ];
    for ((_, row), (authored, status)) in links.into_iter().zip(expected) {
        assert_eq!(
            string(row, "/payload/authored_path"),
            authored,
            "original link spelling"
        );
        assert_eq!(
            string(row, "/payload/reference/resolution/status"),
            status,
            "resolution for {authored}"
        );
        if status == "resolved" {
            assert_eq!(
                row.data
                    .pointer("/payload/reference/resolution/target/source/entry/entry_index"),
                Some(&json!(child)),
                "both original links address one selected physical child"
            );
        }
    }
    Ok(())
}

fn verify_include_link(archive: &Archive, rows: &[MediaRecord]) -> TestResult {
    let (_, link) = typed(rows, "link")
        .find(|(_, row)| string(row, "/payload/kind") == "character-parameters-include")
        .ok_or("authored character parameters include")?;
    assert_eq!(
        entry_path(link),
        PARAM_PATH,
        "include is owned by the original parameters"
    );
    assert_eq!(
        string(link, "/payload/authored_path"),
        INCLUDED_PARAMS,
        "preserve the rooted include selector"
    );
    assert_eq!(
        link.data
            .pointer("/payload/reference/resolution/target/source/entry/entry_index"),
        Some(&json!(entry_index(archive, INCLUDED_PARAMS)?)),
        "include link resolves to the selected physical document"
    );
    Ok(())
}

fn typed<'a>(
    rows: &'a [MediaRecord],
    kind: &'a str,
) -> impl Iterator<Item = (usize, &'a MediaRecord)> {
    rows.iter()
        .enumerate()
        .filter(move |(_, row)| string(row, "/record_type") == kind)
}

fn entry_path(row: &MediaRecord) -> &str {
    match &row.source.identity {
        Identity::Entry { entry, .. } => &entry.path,
        _ => "",
    }
}

fn string<'a>(row: &'a MediaRecord, key: &str) -> &'a str {
    row.data
        .pointer(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn entry_index(archive: &Archive, path: &str) -> Result<usize, Box<dyn Error>> {
    archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .map(p4kx_extraction::Entry::index)
        .ok_or_else(|| format!("original Entry {path}").into())
}
