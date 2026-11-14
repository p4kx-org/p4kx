// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry, test_key,
};

use super::{TestResult, verify};

#[test]
fn standalone_definition_graph_replays_text_and_binary_in_physical_archive_scopes() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("graph.p4k");
    let branches = br#"<AnimDB TagDef="Animations/tag.xml"><SubADBs>
        <SubADB File="animations/CHILD.adb" Tags="Same"><FragmentID Name="Run"/></SubADB>
        <SubADB File="Animations/blank.adb"/><SubADB File="Animations/missing.adb"/>
        </SubADBs></AnimDB>"#;
    let binary = TestElement::new("AnimDB")
        .child(
            TestElement::new("FragmentList").child(
                TestElement::new("Run").child(
                    TestElement::new("Fragment")
                        .attribute("GUID", "a")
                        .attribute("GUID", "")
                        .child(
                            TestElement::new("AnimLayer")
                                .child(TestElement::new("Animation").attribute("name", "opaque")),
                        ),
                ),
            ),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let archive = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Animations/root.adb", branches))
        .entry(TestEntry::stored("Data/Animations/child.adb", binary.as_slice()).encrypted())
        .entry(TestEntry::stored("Data/Animations/blank.adb", b"<Other/>"))
        .entry(TestEntry::stored(
            "Data/Animations/tag.xml",
            b"<TagDefinition/>",
        ));
    let nested = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Animations/root.adb", branches))
        .entry(TestEntry::stored("Data/Animations/child.adb", b"<AnimDB/>"))
        .entry(TestEntry::stored("Data/Animations/missing.adb", b"<AnimDB/>"))
        .entry(TestEntry::stored("Data/Animations/one.bspace", br#"<ParaGroup>
            <ExampleList><Example AName="loose" Alias="kept"/><Example AName="absent"/></ExampleList>
            <Blendable><Face p0="kept"/></Blendable>
            </ParaGroup>"#))
        .entry(TestEntry::stored("Data/Animations/one.comb", br#"<CombinedBlendSpace>
            <BlendSpaces><BlendSpace AName="Animations/one.bspace"/><BlendSpace AName="Animations/../refused.bspace"/></BlendSpaces>
            </CombinedBlendSpace>"#))
        .entry(TestEntry::stored("Data/Animations/loose.caf", b"unselected CAF payload"))
        .build();
    archive
        .entry(TestEntry::stored("Data/Animations/graph.pak", nested))
        .write_to(&path)?;
    verify(&Archive::open_with_key(path, Some(test_key()))?, true)
}
