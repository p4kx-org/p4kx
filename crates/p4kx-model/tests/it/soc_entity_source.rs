// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::{ChunkFile, ChunkType, Container};
use p4kx_cryxml::{SourceAttributePosition, SourceDocument, SourceReadOptions};
use p4kx_model::{SocEntities, SocEntity, SocEntityFile, SocEntitySourceError};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestChunkFile, TestElement};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ENTITIES: u32 = 0x0004;
const CLASS: &str = "00000000-0000-0000-0000-000010630500";

fn binary_entities() -> Vec<u8> {
    cryxml(
        &TestElement::new("Entities")
            .child(entity("class-only", "1,2,3"))
            .child(TestElement::new("Metadata"))
            .child(entity("own-geometry", "4,5,6").child(geometry()))
            .child(entity("invalid-transform", "bad,0,0")),
    )
}

fn cryxml(root: &TestElement) -> Vec<u8> {
    root.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

fn entity(name: &str, position: &str) -> TestElement {
    TestElement::new("Entity")
        .attribute("Name", name)
        .attribute("EntityClass", "EntityComponent")
        .attribute("EntityClassGUID", CLASS)
        .attribute("Pos", position)
        .attribute("Rotate", "1,0,0,0")
        .attribute("Scale", "2,3,4")
}

fn geometry() -> TestElement {
    TestElement::new("PropertiesDataCore").child(
        TestElement::new("EntityGeometryResource").child(
            TestElement::new("Geometry").child(TestElement::new("Geometry").child(
                TestElement::new("Geometry").attribute("path", "objects/fixture/model.cgf"),
            )),
        ),
    )
}

fn soc_bytes() -> Vec<u8> {
    TestChunkFile::crch()
        .chunk(0x0001, 5, vec![0; 4])
        .chunk(ENTITIES, 3, binary_entities())
        .bytes()
}

#[test]
fn source_coverage_includes_class_guid_on_entities_with_own_geometry() -> TestResult {
    let bytes = soc_bytes();
    let chunks = ChunkFile::parse(bytes.as_slice())?;
    let [(index, header)] = chunks
        .chunks_of(ChunkType::new(Container::CrCh, ENTITIES))
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "one original entity chunk")?;
    assert_eq!(
        index, 1,
        "original table index includes preceding non-entity chunks"
    );
    let data = chunks
        .chunk_data(index)
        .ok_or("original entity chunk bytes")?;
    let document = SourceDocument::parse(data, SourceReadOptions::default())?;
    let authored = SocEntityFile::parse(&bytes)?;
    let [chunk] = authored
        .entity_chunks()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "one observed entity chunk")?;
    assert_eq!(
        chunk.chunk_index(),
        index,
        "source chunk uses the original table index"
    );
    assert_eq!(
        chunk.header().offset(),
        header.offset(),
        "source position has the original chunk base"
    );
    let observed: Vec<_> = chunk.entities().collect();
    verify_slots(&observed, &document)?;
    verify_transforms(&observed)?;
    let rendered = SocEntities::read(&bytes)?.ok_or("original renderer result")?;
    assert_eq!(
        rendered.classed.len(),
        2,
        "the render view lists class geometry lookup candidates"
    );
    assert_eq!(
        rendered.props.len(),
        1,
        "the render view places the entity with its own geometry"
    );
    Ok(())
}

fn verify_slots(observed: &[SocEntity<'_>], document: &SourceDocument<'_>) -> TestResult {
    let original: Vec<_> = document
        .root()
        .elements()
        .enumerate()
        .filter(|(_, node)| node.attribute("EntityClassGUID") == Some(CLASS))
        .collect();
    assert_eq!(
        observed.len(),
        original.len(),
        "every original class occurrence is retained"
    );
    assert_eq!(
        observed
            .iter()
            .map(|entry| entry.ordinal())
            .collect::<Vec<_>>(),
        [0, 2, 3],
        "physical element slots include other direct children"
    );
    for (entry, (ordinal, node)) in observed.iter().zip(original) {
        let class = entry
            .element()
            .attributes()
            .find(|attribute| attribute.name() == "EntityClassGUID")
            .ok_or("original class selector")?;
        let original = node
            .attributes()
            .find(|attribute| attribute.name() == "EntityClassGUID")
            .ok_or("independent class selector coordinate")?;
        assert_eq!(
            entry.element().position(),
            node.position(),
            "source element position for physical slot {ordinal}"
        );
        assert_eq!(
            class.value(),
            CLASS,
            "original selector for physical slot {ordinal}"
        );
        assert_eq!(
            class.position(),
            original.position(),
            "source attribute coordinate at slot {ordinal}"
        );
    }
    Ok(())
}

fn verify_transforms(observed: &[SocEntity<'_>]) -> TestResult {
    let own = observed.get(1).ok_or("entity with its own geometry")?;
    let local = own
        .local_transform()
        .ok_or("valid authored local transform")?;
    assert_eq!(
        local.translation(),
        [4.0, 5.0, 6.0],
        "entity position is container-local"
    );
    assert_eq!(
        [local.rows[0][0], local.rows[1][1], local.rows[2][2]],
        [2.0, 3.0, 4.0],
        "original axis scale is retained"
    );
    let invalid = observed.get(2).ok_or("original malformed transform")?;
    assert!(
        invalid.local_transform().is_none(),
        "the class occurrence survives an unusable transform"
    );
    assert_eq!(
        invalid.element().attribute("Pos"),
        Some("bad,0,0"),
        "the original failed field remains inspectable"
    );
    Ok(())
}

#[test]
fn text_attributes_retain_exact_chunk_relative_lexical_ranges() -> TestResult {
    let bytes = TestChunkFile::crch()
        .chunk(ENTITIES, 3, b"<Other/>".as_slice())
        .chunk(
            ENTITIES,
            3,
            br#"<Entities><Entity Name="A&amp;B"/></Entities>"#.as_slice(),
        )
        .bytes();
    let file = SocEntityFile::parse(&bytes)?;
    let mut chunks = file.entity_chunks();
    assert!(
        matches!(
            chunks.next(),
            Some(Err(SocEntitySourceError::Root { chunk_index: 0 }))
        ),
        "the first malformed entity chunk reports its actual table index"
    );
    let chunk = chunks.next().ok_or("next original entity chunk")??;
    assert_eq!(
        chunk.chunk_index(),
        1,
        "a failed chunk leaves the next occurrence readable"
    );
    let entity = chunk.entities().next().ok_or("original text entity")?;
    let name = entity
        .element()
        .attributes()
        .next()
        .ok_or("original name attribute")?;
    let SourceAttributePosition::Text(range) = name.position() else {
        return Err("expected original text attribute range".into());
    };
    let offset = usize::try_from(chunk.header().offset())?;
    let spelling = bytes
        .get(offset + range.start..offset + range.end)
        .ok_or("attribute within original SOC bytes")?;
    assert_eq!(
        spelling, br#"Name="A&amp;B""#,
        "the original spelling belongs to the same chunk"
    );
    assert_eq!(
        name.value(),
        "A&B",
        "the decoded text remains available independently"
    );
    Ok(())
}

#[test]
fn duplicate_transform_attributes_keep_both_source_slots() -> TestResult {
    let xml = cryxml(
        &TestElement::new("Entities")
            .child(entity("duplicate-pos", "1,2,3").attribute("Pos", "7,8,9")),
    );
    let bytes = TestChunkFile::crch().chunk(ENTITIES, 3, xml).bytes();
    let source = SocEntityFile::parse(&bytes)?;
    let chunk = source
        .entity_chunks()
        .next()
        .ok_or("entity source chunk")??;
    let entity = chunk
        .entities()
        .next()
        .ok_or("entity with two original positions")?;
    let values: Vec<_> = entity
        .element()
        .attributes()
        .filter(|attribute| attribute.name() == "Pos")
        .collect();
    let [first, second] = values.as_slice() else {
        return Err("both original position fields".into());
    };
    assert_eq!(
        [first.value(), second.value()],
        ["1,2,3", "7,8,9"],
        "the source keeps order and both authored values"
    );
    assert_ne!(
        first.position(),
        second.position(),
        "each authored position has a distinct coordinate"
    );
    assert!(
        entity.local_transform().is_none(),
        "an ambiguous transform has no derived matrix"
    );
    Ok(())
}

#[test]
fn source_file_distinguishes_an_empty_soc_from_unrelated_chunk_framing() -> TestResult {
    let empty = TestChunkFile::crch().bytes();
    let soc = SocEntityFile::parse(&empty)?;
    assert_eq!(
        soc.entity_chunks().count(),
        0,
        "a readable SOC may contain no entities"
    );
    let ivo = TestChunkFile::ivo().bytes();
    assert!(
        SocEntityFile::parse(&ivo).is_err(),
        "an Ivo file cannot establish that an SOC has no entities"
    );
    Ok(())
}
