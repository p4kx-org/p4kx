// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Exclusion, Filter};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord};
use p4kx_testkit::{
    DataForgeBuilder, TestAnimation, TestDba, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry,
    TestModel, TestPort, entity_schema, landing_system_record,
};
use serde_json::Value;

use super::support::{self, TestResult};

#[path = "animation_scope_receipt_assertions.rs"]
mod receipt_assertions;

const CDF: &str = "Data/Objects/selected.cdf";
const CHR: &str = "Data/Objects/shared.chr";
const BLOCKED_CDF: &str = "Data/Objects/blocked.cdf";
const STATIC_MODEL: &str = "Data/Objects/static.cgf";
const BLOCKED: &str = "Data/Animations/blocked.animevents";
const UNRELATED: &str = "Data/Objects/unrelated.chrparams";
const STREAM: &str = "animation-definition.jsonl";

#[test]
fn scoped_animation_definitions_follow_authored_targets_beyond_the_include() -> TestResult {
    let temp = tempfile::tempdir()?;
    let archive = fixture(temp.path(), false)?;
    for by_entity in [false, true] {
        let out = temp.path().join(if by_entity { "entity" } else { "model" });
        let index = support::published(p4kx_media::export(&archive, &out, &options(by_entity)?)?)?;
        let rows = support::records(&out, &index, STREAM)?;
        assert_eq!(
            index.source_entries.written,
            if by_entity { 3 } else { 2 },
            "count dependencies and independently included sources once"
        );
        assert_eq!(
            rows.iter()
                .filter(|row| row.subresource.namespace == "animation-definition-document")
                .count(),
            2,
            "only the parameters and authored event database receive definition callbacks"
        );
        verify_links(&rows)?;
    }
    Ok(())
}

#[test]
fn unavailable_rig_prevents_complete_scope_and_exclusion_keeps_the_other_branches() -> TestResult {
    let temp = tempfile::tempdir()?;
    let archive = fixture(temp.path(), true)?;
    let out = temp.path().join("blocked-scope");
    let mut selection = options(true)?;
    assert!(
        matches!(p4kx_media::export(&archive, &out, &selection), Err(error)
            if error.to_string().contains("Archive key required")
                && error.to_string().contains("blocked.cdf")),
        "a readable root cannot hide its blocked character definition"
    );
    assert!(
        !out.join("media-index.json").exists(),
        "blocked scope leaves no published index"
    );
    selection.exclude = Exclusion::parse([BLOCKED, BLOCKED_CDF])?;
    let index = support::published(p4kx_media::export(&archive, &out, &selection)?)?;
    let rows = support::records(&out, &index, STREAM)?;
    assert_eq!(
        index.source_entries.written, 3,
        "explicit exclusion leaves the other branches"
    );
    verify_links(&rows)?;
    receipt_assertions::verify_scope_receipts(&archive, &rows, &out.join(&index.artifact_base))?;

    let mut selection = options(false)?;
    selection.exclude = Exclusion::parse([CDF])?;
    let excluded_out = temp.path().join("excluded-root");
    let index = support::published(p4kx_media::export(&archive, &excluded_out, &selection)?)?;
    assert_eq!(
        index.source_entries.written, 0,
        "explicit exclusion of the root admits no definitions"
    );
    let rows = support::records(&excluded_out, &index, STREAM)?;
    receipt_assertions::verify_root_exclusion(&archive, &rows)?;
    Ok(())
}

fn options(by_entity: bool) -> Result<ExportOptions, Box<dyn std::error::Error>> {
    let mut options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        include: Some(Filter::parse(UNRELATED)?),
        exclude: Exclusion::parse([BLOCKED])?,
        ..ExportOptions::default()
    };
    if by_entity {
        options.animation_entity = Some("FixtureRoot".into());
        options.kinds.push(MediaKind::Archive);
    } else {
        options.animation_model = Some(CDF.into());
    }
    Ok(options)
}

fn verify_links(rows: &[MediaRecord]) -> TestResult {
    let mut event_links = rows
        .iter()
        .filter(|row| {
            row.data.pointer("/payload/kind").and_then(Value::as_str) == Some("event-database")
        })
        .filter_map(|row| {
            row.data
                .pointer("/payload/reference/resolution/status")
                .and_then(Value::as_str)
        });
    assert_eq!(
        event_links.next(),
        Some("resolved"),
        "authored dependency outranks an include mismatch"
    );
    assert_eq!(
        event_links.next(),
        Some("not-selected"),
        "explicit exclusion remains authoritative"
    );
    assert_eq!(
        event_links.next(),
        None,
        "no extra database link is synthesized"
    );
    let bank = rows
        .iter()
        .find(|row| row.subresource.namespace == "animation-definition-named-dba-binding")
        .ok_or("scoped exact CAF must have a tracks database binding")?;
    assert_eq!(
        bank.data.pointer("/payload/state").and_then(Value::as_str),
        Some("bank-match"),
        "exact scoped CAF path selects the original DBA metadata"
    );
    assert!(
        rows.iter()
            .any(|row| row.subresource.namespace == "animation-definition-event-timeline"),
        "declared event database produces its timeline"
    );
    Ok(())
}

fn fixture(root: &Path, blocked_branch: bool) -> Result<Archive, Box<dyn std::error::Error>> {
    let bank = TestDba::new()
        .animation(TestAnimation::new("Animations/Scope/clip.caf", 30))
        .bytes();
    support::archive_with(
        root,
        [
            TestEntry::stored("Data/Game2.dcb", database(blocked_branch)),
            TestEntry::stored(BLOCKED_CDF, b"encrypted fixture").encrypted(),
            TestEntry::stored(STATIC_MODEL, TestModel::cube().cgf()),
            TestEntry::stored(CDF, br#"<CharacterDefinition><Model File="Objects/shared.chr"/></CharacterDefinition>"#),
            TestEntry::stored(CHR, TestModel::cube().cgf()),
            TestEntry::stored(
                "Data/Objects/shared.chrparams",
                br##"<Params><AnimationList>
                <Animation name="#filepath" path="Animations/Scope"/>
                <Animation name="concrete" path="clip.caf"/>
                <Animation name="wild*" path="*.caf"/>
                <Animation name="$TracksDatabase" path="Animations/only.dba"/>
                <Animation name="$AnimEventDatabase" path="Animations/linked.animevents"/>
                <Animation name="$AnimEventDatabase" path="Animations/blocked.animevents"/>
                </AnimationList></Params>"##,
            ),
            TestEntry::stored(UNRELATED, b"<Params/>"),
            TestEntry::stored("Data/Animations/only.dba", bank),
            TestEntry::stored(
                "Data/Animations/linked.animevents",
                br#"<anim_event_list><animation name="Animations/Scope/clip.caf"><event name="cue" time="0.5"/></animation></anim_event_list>"#,
            ),
            TestEntry::stored(BLOCKED, b"the excluded bytes are intentionally malformed"),
        ],
    )
}

fn database(blocked_branch: bool) -> Vec<u8> {
    let root = TestEntity::new(1, "FixtureRoot", "entities/fixture_root.xml")
        .geometry("Objects/selected.cdf", None);
    let builder = entity_schema(DataForgeBuilder::new());
    if !blocked_branch {
        return builder.record(root.record()).build();
    }
    let root = root
        .vehicle("", "")
        .landing_system(6)
        .loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("blocked", "FixtureBlocked"),
            TestLoadoutEntry::class("static", "FixtureStatic"),
            TestLoadoutEntry::class("sharedLeft", "FixtureShared"),
            TestLoadoutEntry::class("sharedRight", "FixtureShared"),
            TestLoadoutEntry::class("empty", "FixtureEmpty"),
        ]))
        .port(TestPort::new("defaultEmpty").default_item(5));
    let blocked = TestEntity::new(2, "FixtureBlocked", "entities/fixture_blocked.xml")
        .geometry("Objects/blocked.cdf", None);
    let static_model = TestEntity::new(3, "FixtureStatic", "entities/fixture_static.xml")
        .geometry("Objects/static.cgf", None);
    builder
        .record(root.record())
        .record(blocked.record())
        .record(static_model.record())
        .record(
            TestEntity::new(4, "FixtureShared", "entities/fixture_shared.xml")
                .geometry("Objects/selected.cdf", None)
                .record(),
        )
        .record(TestEntity::new(5, "FixtureEmpty", "entities/fixture_empty.xml").record())
        .record(landing_system_record(
            6,
            "FixtureLanding",
            &[("strut", "Objects/shared.chr", 5.0)],
        ))
        .build()
}

#[test]
fn malformed_filepath_does_not_admit_a_stale_scope_candidate() -> TestResult {
    let temp = tempfile::tempdir()?;
    let archive = malformed_filepath_fixture(temp.path())?;
    let out = temp.path().join("selectors");
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition, MediaKind::Animation],
        include: Some(Filter::parse(UNRELATED)?),
        animation_model: Some(CHR.into()),
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &out, &options)?)?;
    let rows = support::records(&out, &index, STREAM)?;
    let selected: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "animation-definition-event-timeline")
        .filter_map(|row| {
            row.data
                .pointer("/payload/target/reference/resolution/status")
                .and_then(Value::as_str)
        })
        .collect();
    assert_eq!(
        selected,
        ["resolved", "not-selected", "resolved"],
        "only usable ordered CAF targets enter the physical scope"
    );
    assert_eq!(
        index.source_entries.written, 2,
        "an ambiguous event path does not select its decoy document"
    );
    Ok(())
}

fn malformed_filepath_fixture(root: &Path) -> Result<Archive, Box<dyn std::error::Error>> {
    support::archive_with(
        root,
        [
            TestEntry::stored(CHR, TestModel::cube().cgf()),
            TestEntry::stored(
                "Data/Objects/shared.chrparams",
                br##"<Params><AnimationList>
                <Animation name="#filepath" path="Animations/First"/>
                <Animation name="before" path="kept.caf"/>
                <Animation name="#filepath"/>
                <Animation name="after" path="stale.caf"/>
                <Animation name="#filepath" path="Animations/Second"/>
                <Animation name="restored" path="found.caf"/>
                <Animation name="$AnimEventDatabase" path="Animations/scope.animevents"/>
                <Animation name="$AnimEventDatabase" path="Animations/decoy.animevents" Path="Animations/other.animevents"/>
                </AnimationList></Params>"##,
            ),
            TestEntry::stored("Data/Animations/First/kept.caf", b"actual scoped source"),
            TestEntry::stored("Data/Animations/First/stale.caf", b"old inactive scope"),
            TestEntry::stored("Data/Animations/Second/found.caf", b"actual restored scope"),
            TestEntry::stored(
                "Data/Animations/scope.animevents",
                br#"<anim_event_list>
                <animation name="Animations/First/kept.caf"/>
                <animation name="Animations/First/stale.caf"/>
                <animation name="Animations/Second/found.caf"/>
                </anim_event_list>"#,
            ),
            TestEntry::stored("Data/Animations/decoy.animevents", b"ambiguous source no read"),
        ],
    )
}
