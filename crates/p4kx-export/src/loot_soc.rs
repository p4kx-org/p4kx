// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use p4kx_extraction::chunks::{ChunkFile, ChunkType, Container};
use p4kx_extraction::{Guid, SourceLocation};

use crate::starmap_xml::{Document, Node, Root, XmlError};

/// The chunk of a `.soc` file that holds its entities as `CryXmlB`.
pub(crate) const ENTITIES: ChunkType = ChunkType::new(Container::CrCh, 0x0004);

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
    #[error("its .soc TagId has no position in the parsed XML")]
    UnboundTag,
}

/// How many entities there are and how many carry each tag.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EntityTags {
    pub(crate) entities: u32,
    pub(crate) by_tag: HashMap<Guid, u32>,
}

pub(crate) struct CountedSoc<'bytes> {
    pub(crate) file: ChunkFile<&'bytes [u8]>,
    pub(crate) first_tags: HashMap<Guid, TagSite>,
}

pub(crate) struct TagSite {
    chunk: u64,
    bytes: Range<u64>,
    node: u32,
    attribute: u64,
    name: Box<str>,
    format: TagXmlFormat,
}

#[derive(Clone, Copy)]
enum TagXmlFormat {
    CryXml,
    Text,
}

impl TagSite {
    pub(crate) fn location(self) -> SourceLocation {
        let namespace = match self.format {
            TagXmlFormat::CryXml => "socEntityTag.cryXml",
            TagXmlFormat::Text => "socEntityTag.text",
        };
        SourceLocation::Subresource {
            namespace: namespace.to_owned(),
            indices: vec![
                self.chunk,
                self.bytes.start,
                self.bytes.end,
                u64::from(self.node),
                self.attribute,
            ],
            field: format!("@{}", self.name),
        }
    }
}

struct TagChunk<'doc, 'bytes> {
    index: u64,
    bytes: Range<u64>,
    document: &'doc Document<'bytes>,
}

impl EntityTags {
    /// Adds the entities and their tags and retains the original parsed chunk table.
    ///
    /// # Errors
    ///
    /// Returns why the file cannot be read; the counts then hold a part of the file.
    pub(crate) fn count_soc<'bytes>(
        &mut self,
        bytes: &'bytes [u8],
    ) -> Result<CountedSoc<'bytes>, SocError> {
        let file = ChunkFile::parse(bytes).map_err(|_| SocError::NotChunkFile)?;
        let mut first_tags = HashMap::new();
        for (index, chunk) in file.chunks_of(ENTITIES) {
            let data = file.chunk_data(index).ok_or(SocError::ChunkOutOfRange)?;
            let end = chunk
                .offset()
                .checked_add(chunk.size())
                .ok_or(SocError::ChunkOutOfRange)?;
            let document = Document::parse(data)?;
            let index = u64::try_from(index).map_err(|_| SocError::UnboundTag)?;
            let input = TagChunk {
                index,
                bytes: chunk.offset()..end,
                document: &document,
            };
            self.add_entities(document.root(), |id, node, attribute, name| {
                record_first(&mut first_tags, id, &input, node, attribute, name)
            })?;
        }
        Ok(CountedSoc { file, first_tags })
    }

    #[cfg(test)]
    fn add_soc(&mut self, bytes: &[u8]) -> Result<(), SocError> {
        self.count_soc(bytes).map(|_| ())
    }

    #[cfg(test)]
    fn add_xml(&mut self, bytes: &[u8]) -> Result<(), SocError> {
        self.add_entities(Document::parse(bytes)?.root(), |_, _, _, _| Ok(()))
    }

    fn add_entities<'xml>(
        &mut self,
        root: Root<'xml>,
        mut observe: impl FnMut(Guid, Root<'xml>, usize, &'xml str) -> Result<(), SocError>,
    ) -> Result<(), SocError> {
        let mut entity_tags = HashSet::new();
        for entity in root
            .children()
            .filter(|child| child.name().eq_ignore_ascii_case("Entity"))
        {
            increment(&mut self.entities)?;
            entity_tags.clear();
            for tag in entity
                .children()
                .filter(|child| child.name().eq_ignore_ascii_case("Tags"))
                .flat_map(Node::children)
                .filter(|child| child.name().eq_ignore_ascii_case("Tag"))
            {
                let Some((ordinal, (name, authored))) = tag
                    .attributes()
                    .enumerate()
                    .find(|(_, (name, _))| name.eq_ignore_ascii_case("TagId"))
                else {
                    continue;
                };
                let Ok(id) = authored.trim().parse::<Guid>() else {
                    continue;
                };
                if entity_tags.contains(&id) {
                    continue;
                }
                entity_tags
                    .try_reserve(1)
                    .map_err(|_| SocError::TagAllocation)?;
                entity_tags.insert(id);
                self.count_tag(id)?;
                observe(id, tag, ordinal, name)?;
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

fn record_first(
    sites: &mut HashMap<Guid, TagSite>,
    guid: Guid,
    input: &TagChunk<'_, '_>,
    tag: Root<'_>,
    attribute: usize,
    name: &str,
) -> Result<(), SocError> {
    if sites.contains_key(&guid) {
        return Ok(());
    }
    let attribute = u64::try_from(attribute).map_err(|_| SocError::UnboundTag)?;
    let node = input
        .document
        .original_index(tag)
        .ok_or(SocError::UnboundTag)?;
    let format = match tag {
        Root::Cry(_) => TagXmlFormat::CryXml,
        Root::Text(_) => TagXmlFormat::Text,
    };
    sites.try_reserve(1).map_err(|_| SocError::TagAllocation)?;
    sites.insert(
        guid,
        TagSite {
            chunk: input.index,
            bytes: input.bytes.clone(),
            node,
            attribute,
            name: name.into(),
            format,
        },
    );
    Ok(())
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
