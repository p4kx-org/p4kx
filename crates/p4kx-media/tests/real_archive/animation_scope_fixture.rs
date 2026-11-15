// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, OpenOptions};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntity,
    TestEntry, TestInstance, TestLoadout, TestModel, TestStruct, TestType, TestValue,
    entity_schema, test_key, write_build_manifest,
};

use super::sources::Closure;
use super::{Selection, TestResult, verify};

const MODEL: &str = "Data/Objects/witness.cdf";
const ENTITY: &str = "FixtureScope";
const CHILD: &str = "FixtureAttachment";
const ROOT_ADB: &str = "Data/Animations/Mannequin/witness.adb";
const ATTACHED_ADB: &str = "Data/Animations/Mannequin/attached.adb";
const SUB_ADB: &str = "Data/Animations/Mannequin/sub.adb";
const INCLUDE: &str = "Data/Objects/witness-shared.chrparams";
const COMBINED: &str = "Data/Animations/Blend/locomotion/Run.comb";
const BLEND_SPACE: &str = "Data/Animations/Blend/run.bspace";
const MANNEQUIN: &[u8] = br#"<AnimDB><FragmentList><Idle><Fragment><AnimLayer>
    <Animation name="clip"/>
    <Animation name="clip"/>
    <Animation name=""/>
    <Animation xmlns:other="urn:fixture" other:name="not-an-authored-selector"/>
    </AnimLayer></Fragment></Idle></FragmentList></AnimDB>"#;
const ANIMATED: &str = "SAnimatedCharacterComponentParams";
const CONTROLLER: &str = "SAnimationControllerParams";

#[test]
fn synthetic_animation_scope_witness_checks_model_and_entity_publications() -> TestResult {
    let directory = tempfile::tempdir()?;
    write_build_manifest(directory.path(), "fixture", "1.2.3", "42", "scope-fixture")?;
    let archive = archive(directory.path())?;
    let build = archive.build().ok_or("fixture Build")?;
    let watch = ArchiveWatch::new(&archive);
    for selection in [
        Selection::Model(MODEL.to_owned()),
        Selection::Entity(ENTITY.to_owned()),
    ] {
        let expected = selection.discover(&archive, &watch)?;
        assert!(
            expected.has_events(),
            "the synthetic scope contains an authored event database"
        );
        for target in [INCLUDE, COMBINED, BLEND_SPACE] {
            assert!(
                expected
                    .documents
                    .values()
                    .any(|source| source.entry.path() == target),
                "the original scope independently reaches {target}"
            );
        }
        let entity = matches!(selection, Selection::Entity(_));
        if entity {
            verify_entity_fixture(&expected)?;
        }
        let blend_entry = expected
            .documents
            .values()
            .find(|source| source.entry.path() == BLEND_SPACE)
            .ok_or("independently reachable BlendSpace source")?
            .entry
            .index();
        let placements = expected.placements.len();
        let matches = verify(&archive, build, &selection, expected)?;
        assert_eq!(
            matches.blend_matches.get(&blend_entry),
            Some(&placements),
            "each placement in the model or Entity scope joins the reachable BlendSpace example"
        );
        assert_eq!(
            matches.database_matches,
            if entity { 6 } else { 0 },
            "the two authored ADB clip occurrences join on each of the three Entity placements"
        );
    }
    watch.check()?;
    Ok(())
}

fn verify_entity_fixture(expected: &Closure<'_>) -> TestResult {
    assert_eq!(
        expected.databases.len(),
        3,
        "one root and two attachment bindings"
    );
    let adb = expected
        .documents
        .values()
        .filter(|document| document.family() == "adb");
    assert_eq!(
        adb.clone().count(),
        3,
        "two authored roots and their one shared transitive database"
    );
    assert_eq!(
        adb.map(|doc| doc.clips.remaining()).sum::<usize>(),
        3,
        "the selected database declares two repeated aliases and an empty selector"
    );
    let [_, left, right] = expected.placements.as_slice() else {
        return Err("the original XML resolves both attachment occurrences".into());
    };
    for placement in [left, right] {
        assert_eq!(placement.evidence.names.len(), 1, "one XML port name");
        assert_eq!(
            placement.evidence.selection.len(),
            2,
            "the original Include and class selectors"
        );
    }
    Ok(())
}

fn dataforge() -> Vec<u8> {
    let entity = TestEntity::new(1, ENTITY, "entities/fixture_scope.xml")
        .geometry("Objects/witness.cdf", None)
        .loadout(TestLoadout::Xml(r"Scripts\Loadouts\witness.xml".into()))
        .component(locator(
            ANIMATED,
            "animDatabase3P",
            r" ANIMATIONS\Mannequin\Witness.ADB ",
        ));
    let child = TestEntity::new(2, CHILD, "entities/fixture_attachment.xml")
        .geometry("Objects/witness.chr", None)
        .component(locator(
            CONTROLLER,
            "AnimationDatabase",
            "Animations/Mannequin/attached.adb",
        ));
    entity_schema(DataForgeBuilder::new())
        .structure(
            TestStruct::new(ANIMATED)
                .parent("DataForgeComponentParams")
                .field("animDatabase3P", TestType::String),
        )
        .structure(
            TestStruct::new(CONTROLLER)
                .parent("DataForgeComponentParams")
                .field("AnimationDatabase", TestType::String),
        )
        .record(entity.record())
        .record(child.record())
        .build()
}

fn archive(root: &Path) -> TestResult<Archive> {
    let path = root.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", dataforge()))
        .entry(TestEntry::stored(
            "Data/Scripts/Loadouts/witness.xml",
            br#"<Loadout><Loadouts><Loadout loadout="Scripts/Loadouts/pArTs.xml"/></Loadouts></Loadout>"#,
        ))
        .entry(TestEntry::stored("Data/Scripts/Loadouts/parts.xml", parts()).encrypted())
        .entry(TestEntry::stored(
            MODEL,
            br#"<CharacterDefinition><Model File="Objects/witness.chr"/></CharacterDefinition>"#,
        ))
        .entry(TestEntry::stored(
            "Data/Objects/witness.chr",
            TestModel::cube().cgf(),
        ))
        .entry(TestEntry::stored(
            "Data/Objects/witness.chrparams",
            br#"<Params><AnimationList>
                <Animation name="clip" path="Animations/clip.caf"/>
                <Animation name="$AnimEventDatabase" path="Animations/witness.animevents"/>
                <Animation name="$Include" path="Data/Objects/witness-shared.chrparams"/>
                </AnimationList></Params>"#,
        ))
        .entry(TestEntry::stored(INCLUDE, shared_parameters()).encrypted())
        .entry(TestEntry::stored(
            COMBINED,
            br#"<CombinedBlendSpace><BlendSpaces>
                <BlendSpace AName="Animations/Blend/run.bspace"/>
                <BlendSpace AName="Data/Animations/Blend/RUN.bspace"/>
                </BlendSpaces></CombinedBlendSpace>"#,
        ))
        .entry(TestEntry::stored(
            BLEND_SPACE,
            br#"<ParaGroup><ExampleList><Example AName="clip"/></ExampleList></ParaGroup>"#,
        ))
        .entry(TestEntry::stored(
            "Data/Animations/witness.animevents",
            br#"<anim_event_list><animation name="Animations/clip.caf">
                <event name="cue" time="0.5"/></animation></anim_event_list>"#,
        ))
        .entry(TestEntry::stored(
            "Data/Animations/clip.caf",
            b"reference target",
        ))
        .entry(TestEntry::stored(ROOT_ADB, br#"<AnimDB><SubADBs><SubADB File="Animations/Mannequin/sub.adb"/></SubADBs></AnimDB>"#).encrypted())
        .entry(TestEntry::stored(ATTACHED_ADB, br#"<AnimDB><SubADBs><SubADB File="Data\Animations\Mannequin\Sub.ADB"/></SubADBs></AnimDB>"#))
        .entry(TestEntry::stored(SUB_ADB, MANNEQUIN).encrypted())
        .entry(TestEntry::stored(
            "Data/Objects/unrelated.socpak",
            b"excluded malformed container",
        ))
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn locator(component: &str, field: &str, path: &str) -> TestValue {
    TestValue::Struct(TestInstance::new(component).value(field, TestValue::text(path)))
}

const fn shared_parameters() -> &'static [u8] {
    br##"<Params><AnimationList><Animation Name="#Filepath" Path="Animations/Blend"/>
        <Animation Name="locomotion_*" Path="*/*.comb"/>
        </AnimationList></Params>"##
}

fn parts() -> Vec<u8> {
    let item = |port| {
        TestElement::new("Item")
            .attribute("portName", port)
            .attribute("itemName", CHILD)
    };
    TestElement::new("Loadout")
        .child(
            TestElement::new("Items")
                .child(item("left"))
                .child(item("right")),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}
