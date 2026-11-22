// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::Guid;
use p4kx_extraction::chunks::{ChunkFile, ChunkType, Container};

use crate::starmap_xml::{Document, Node, XmlError, attribute};

/// The chunk of a `.soc` file that holds its entities as `CryXmlB`.
const ENTITIES: ChunkType = ChunkType::new(Container::CrCh, 0x0004);

/// Why a `.soc` file cannot be counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum SocError {
    /// The bytes are no chunk file.
    #[error("its .soc is no chunk file")]
    NotChunkFile,
    /// The chunk table lists a chunk whose data lies outside the file.
    #[error("a chunk of its .soc lies outside the file")]
    ChunkOutOfRange,
    /// An entity chunk is no XML document.
    #[error("an entity chunk of its .soc {0}")]
    Xml(#[from] XmlError),
    /// A count exceeds the catalog's unsigned 32-bit representation.
    #[error("its .soc entity or tag count exceeds the catalog representation")]
    CountOverflow,
    /// A tag collection could not allocate space for a new tag.
    #[error("its .soc tag counts could not be allocated")]
    TagAllocation,
}

/// How many entities there are and how many carry each tag.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EntityTags {
    pub(crate) entities: u32,
    pub(crate) by_tag: HashMap<Guid, u32>,
}

impl EntityTags {
    /// Adds each entity and its distinct tags from the `.soc` file.
    ///
    /// # Errors
    ///
    /// Returns why the file cannot be read; the counts then hold a part of the file.
    pub(crate) fn add_soc(&mut self, bytes: &[u8]) -> Result<(), SocError> {
        let file = ChunkFile::parse(bytes).map_err(|_| SocError::NotChunkFile)?;
        for (index, _) in file.chunks_of(ENTITIES) {
            let data = file.chunk_data(index).ok_or(SocError::ChunkOutOfRange)?;
            self.add_xml(data)?;
        }
        Ok(())
    }

    /// Adds the entities of the XML document `bytes`, `CryXml` or text.
    fn add_xml(&mut self, bytes: &[u8]) -> Result<(), SocError> {
        self.add_entities(Document::parse(bytes)?.root())
    }

    fn add_entities<'d, N: Node<'d>>(&mut self, root: N) -> Result<(), SocError> {
        let mut entity_tags = HashSet::new();
        for entity in root
            .children()
            .filter(|child| child.name().eq_ignore_ascii_case("Entity"))
        {
            increment(&mut self.entities)?;
            entity_tags.clear();
            for id in entity
                .children()
                .filter(|child| child.name().eq_ignore_ascii_case("Tags"))
                .flat_map(Node::children)
                .filter(|child| child.name().eq_ignore_ascii_case("Tag"))
                .filter_map(|tag| attribute(tag, "TagId"))
                .filter_map(|id| id.trim().parse::<Guid>().ok())
            {
                if entity_tags.contains(&id) {
                    continue;
                }
                entity_tags
                    .try_reserve(1)
                    .map_err(|_| SocError::TagAllocation)?;
                entity_tags.insert(id);
                self.count_tag(id)?;
            }
        }
        Ok(())
    }

    fn count_tag(&mut self, tag: Guid) -> Result<(), SocError> {
        if let Some(count) = self.by_tag.get_mut(&tag) {
            return increment(count);
        }
        self.by_tag
            .try_reserve(1)
            .map_err(|_| SocError::TagAllocation)?;
        self.by_tag.insert(tag, 1);
        Ok(())
    }
}

fn increment(count: &mut u32) -> Result<(), SocError> {
    *count = count.checked_add(1).ok_or(SocError::CountOverflow)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAG_A: &str = "00ead468-9073-485b-a6df-ca6b94053603";
    const TAG_B: &str = "59ca5e36-b9a2-4459-ad0e-5745bcf5d899";

    fn guid(text: &str) -> Guid {
        text.parse().expect("a GUID")
    }

    fn counted(xml: &str) -> EntityTags {
        let mut counts = EntityTags::default();
        counts.add_xml(xml.as_bytes()).expect("counts");
        counts
    }

    #[test]
    fn each_tag_is_counted_once_per_entity_that_carries_it() {
        let counts = counted(&format!(
            r#"<Entities>
              <Entity Name="a">
                <Tags><Tag TagId="{TAG_A}"/><Tag TagId="{TAG_A}"/><Tag TagId="{TAG_B}"/></Tags>
                <Tags><Tag TagId="{TAG_A}"/></Tags>
              </Entity>
              <Entity Name="b"><Tags><Tag TagId="{TAG_A}"/></Tags></Entity>
              <Entity Name="c"/>
              <Other/>
            </Entities>"#
        ));
        assert_eq!(counts.entities, 3);
        assert_eq!(counts.by_tag.get(&guid(TAG_A)), Some(&2));
        assert_eq!(counts.by_tag.get(&guid(TAG_B)), Some(&1));
        assert_eq!(counts.by_tag.len(), 2);
    }

    #[test]
    fn a_tag_that_is_no_guid_is_left_out() {
        let counts =
            counted(r#"<Entities><Entity><Tags><Tag TagId="x"/></Tags></Entity></Entities>"#);
        assert_eq!(counts.entities, 1);
        assert!(counts.by_tag.is_empty());
    }

    #[test]
    fn counts_add_up_over_several_documents() {
        let mut counts = EntityTags::default();
        let one =
            format!(r#"<Entities><Entity><Tags><Tag TagId="{TAG_A}"/></Tags></Entity></Entities>"#);
        counts.add_xml(one.as_bytes()).expect("first");
        counts.add_xml(one.as_bytes()).expect("second");
        assert_eq!(counts.entities, 2);
        assert_eq!(counts.by_tag.get(&guid(TAG_A)), Some(&2));
    }

    #[test]
    fn bytes_that_are_no_chunk_file_are_an_error() {
        let mut counts = EntityTags::default();
        assert_eq!(counts.add_soc(b"<Entities/>"), Err(SocError::NotChunkFile));
        assert_eq!(counts.add_soc(b""), Err(SocError::NotChunkFile));
    }

    #[test]
    fn a_chunk_table_that_points_outside_the_file_is_an_error() {
        use p4kx_testkit::TestChunkFile;

        let whole = TestChunkFile::crch()
            .chunk(0x0004, 3, b"<Entities><Entity/></Entities>".to_vec())
            .bytes();
        let mut counts = EntityTags::default();
        assert_eq!(counts.add_soc(&whole), Ok(()), "the whole file counts");
        let cut = whole.get(..whole.len() - 4).expect("a shorter file");
        let mut counts = EntityTags::default();
        assert!(counts.add_soc(cut).is_err(), "the cut file does not");
        assert_eq!(counts, EntityTags::default(), "and adds nothing");
    }

    #[test]
    fn a_chunk_that_is_no_xml_is_an_error() {
        let mut counts = EntityTags::default();
        assert_eq!(
            counts.add_xml(b"\x00\x01 not xml"),
            Err(SocError::Xml(XmlError::Malformed))
        );
    }

    #[test]
    fn entity_counts_preserve_every_representable_value() {
        let mut counts = EntityTags {
            entities: u32::MAX - 1,
            ..EntityTags::default()
        };
        let entity = b"<Entities><Entity/></Entities>";
        assert_eq!(counts.add_xml(entity), Ok(()), "the largest count fits");
        assert_eq!(counts.entities, u32::MAX);
        assert_eq!(
            counts.add_xml(entity),
            Err(SocError::CountOverflow),
            "the next entity would exceed the catalog count"
        );
        assert_eq!(counts.entities, u32::MAX);
    }

    #[test]
    fn a_container_can_count_more_than_sixty_five_thousand_distinct_tags() {
        let mut counts = EntityTags::default();
        for index in 1..=65_536_u128 {
            counts
                .by_tag
                .insert(Guid::from_stored(index.to_le_bytes()), 1);
        }
        let entity =
            format!(r#"<Entities><Entity><Tags><Tag TagId="{TAG_A}"/></Tags></Entity></Entities>"#);
        counts.add_xml(entity.as_bytes()).expect("another tag fits");
        assert_eq!(counts.by_tag.len(), 65_537);
        assert_eq!(counts.by_tag.get(&guid(TAG_A)), Some(&1));
    }
}
