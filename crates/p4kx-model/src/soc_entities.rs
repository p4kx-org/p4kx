// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::{ByteOrder, ChunkFile, ChunkType, Container};
use p4kx_cryxml::{CryXmlError, Xml};
use thiserror::Error;

use crate::Transform;
use crate::mtl_xml::XmlElement;
use crate::soc_geometry::{EntityGeometry, read_geometry};
use crate::soc_light::SocLight;
use crate::soc_light_projection::read_lights;

pub(crate) const ENTITIES: ChunkType = ChunkType::new(Container::CrCh, 0x0004);

/// The most entities one `.soc` file lists.
pub const MAX_SOC_ENTITIES: usize = 1 << 20;

/// The most props and lights one `.soc` file yields, each.
pub const MAX_SOC_PLACED: usize = 1 << 17;

/// A failure to read the entities of a `.soc` file.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SocEntitiesError {
    /// The file is no chunk file this crate reads.
    #[error("the file is not a readable chunk file")]
    Chunks(#[from] p4kx_chunks::ChunkError),
    /// The entity chunk is no readable `CryXml`.
    #[error("the entity chunk is not valid CryXml")]
    CryXml(#[from] CryXmlError),
    /// The entity chunk is no well-formed text XML.
    #[error("the entity chunk is not well-formed XML")]
    Xml(#[from] roxmltree::Error),
    /// The entity chunk nests its elements too deeply to be read.
    #[error("the entity chunk is not readable XML: {0}")]
    TooDeep(#[from] p4kx_cryxml::TextTooDeep),
    /// The entity chunk is flagged big-endian, which is not read.
    #[error("the entity chunk is big-endian")]
    BigEndian,
    /// The file lists more of something than is read.
    #[error("the entity chunk lists more than {limit} {what}")]
    TooMany {
        /// What there is too much of.
        what: &'static str,
        /// The most read.
        limit: usize,
    },
}

/// A prop a `.soc` file places: an entity whose geometry the file names itself
/// (`EntityGeometryResource`).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SocProp {
    /// The entity's name.
    pub name: String,
    /// The model path as the file spells it, below `Data`.
    pub model: String,
    /// The transform from the prop's space to the container's, in `CryEngine` coordinates, Z up
    /// and in metres.
    pub transform: Transform,
    /// The material library the entity chooses (`Material` of its geometry), a path below `Data`
    /// as the file spells it, or `None` when it names none.
    pub material: Option<String>,
    /// The tint palette the entity chooses (`Palette` of its geometry), or `None` when it names
    /// none.
    pub palette: Option<SocPalette>,
}

/// The tint palette an entity of a `.soc` file chooses: the `TintPaletteRef` of its geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SocPalette {
    /// The GUID of the `TintPaletteTree` Record as the file spells it.
    pub root: String,
    /// The path of the child palette below the Record's root; empty for the root.
    pub child: String,
}

/// An entity of a `.soc` file that names the Record of its class (`EntityClassGUID`) and no
/// geometry of its own: its geometry may come from that Record.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SocClassEntity {
    /// The entity's name.
    pub name: String,
    /// The GUID of the entity's class Record as the file spells it.
    pub class: String,
    /// The transform from the entity's space to the container's, or `None` when its position,
    /// rotation or scale is not usable.
    pub transform: Option<Transform>,
    /// The material library the entity chooses itself, which replaces that of its class Record.
    pub material: Option<String>,
    /// The tint palette the entity chooses itself, which replaces that of its class Record.
    pub palette: Option<SocPalette>,
}

/// The props and lights a `.soc` file places: its entity chunk (`CrCh` type `0x0004`, `CryXmlB`).
///
/// Entities that name no geometry of their own and are no lights, e.g. audio triggers and rooms,
/// are only counted in [`other`](Self::other); those that name a class Record are listed in
/// [`classed`](Self::classed) for the caller to look the geometry up.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct SocEntities {
    /// The props with geometry of their own in file order.
    pub props: Vec<SocProp>,
    /// The lights in file order; the lights of a light group follow the group's position.
    pub lights: Vec<SocLight>,
    /// The entities without geometry of their own that name a class Record, lights excluded.
    pub classed: Vec<SocClassEntity>,
    /// The number of entities that are neither a prop, a light nor in [`classed`](Self::classed).
    pub other: usize,
    /// The number of props and lights left out because their transform or parameters are not
    /// usable.
    pub invalid: usize,
}

impl SocEntities {
    /// Reads the entity chunks of the `CrCh` chunk file `bytes`, a `.soc` file, and joins them;
    /// returns `None` when it holds none.
    ///
    /// # Errors
    ///
    /// Returns [`SocEntitiesError`] when the file is no chunk file, a chunk is no XML, or it lists
    /// more than is read.
    pub fn read(bytes: &[u8]) -> Result<Option<Self>, SocEntitiesError> {
        let file = ChunkFile::parse(bytes)?;
        let mut found: Option<Self> = None;
        for (index, chunk) in file.chunks_of(ENTITIES) {
            if chunk.byte_order() == ByteOrder::Big {
                return Err(SocEntitiesError::BigEndian);
            }
            let data = file.chunk_data(index).unwrap_or_default();
            let mut chunk = Self::parse_with_chunk(data, Some(index))?;
            found = Some(match found.take() {
                Some(mut first) => {
                    first.props.append(&mut chunk.props);
                    first.lights.append(&mut chunk.lights);
                    first.classed.append(&mut chunk.classed);
                    first.other += chunk.other;
                    first.invalid += chunk.invalid;
                    for (what, count) in
                        [("props", first.props.len()), ("lights", first.lights.len())]
                    {
                        if count > MAX_SOC_PLACED {
                            return Err(SocEntitiesError::TooMany {
                                what,
                                limit: MAX_SOC_PLACED,
                            });
                        }
                    }
                    if first.classed.len() > MAX_SOC_ENTITIES {
                        return Err(SocEntitiesError::TooMany {
                            what: "entities",
                            limit: MAX_SOC_ENTITIES,
                        });
                    }
                    first
                }
                None => chunk,
            });
        }
        Ok(found)
    }

    /// Reads the entities of the XML document `bytes`, `CryXml` or text, whose root lists
    /// `Entity` elements.
    ///
    /// # Errors
    ///
    /// Returns [`SocEntitiesError`] when `bytes` is no XML or lists more than is read.
    pub fn parse(bytes: &[u8]) -> Result<Self, SocEntitiesError> {
        Self::parse_with_chunk(bytes, None)
    }

    fn parse_with_chunk(bytes: &[u8], chunk: Option<usize>) -> Result<Self, SocEntitiesError> {
        match p4kx_cryxml::read(bytes)? {
            Xml::CryXml(document) => {
                entities(document.root(), chunk.filter(|_| !document.is_lossy()))
            }
            Xml::Text(text) => {
                let text = String::from_utf8_lossy(text);
                p4kx_cryxml::check_text_depth(&text)?;
                let document = roxmltree::Document::parse(&text)?;
                entities(
                    document.root_element(),
                    if matches!(text, std::borrow::Cow::Borrowed(_)) {
                        chunk
                    } else {
                        None
                    },
                )
            }
        }
    }
}

pub(crate) fn child<'a, E: XmlElement<'a>>(parent: E, name: &str) -> Option<E> {
    parent
        .children()
        .find(|element| element.name().eq_ignore_ascii_case(name))
}

pub(crate) fn numbers<const N: usize>(text: Option<&str>) -> Option<[f64; N]> {
    let mut values = [0.0; N];
    let mut parts = text?.split(',');
    for value in &mut values {
        let number: f64 = parts.next()?.trim().parse().ok()?;
        if !number.is_finite() {
            return None;
        }
        *value = number;
    }
    parts.next().is_none().then_some(values)
}

pub(crate) fn number<'a>(element: impl XmlElement<'a>, name: &str) -> Option<f32> {
    let value: f32 = element.attribute(name)?.trim().parse().ok()?;
    value.is_finite().then_some(value)
}

/// Returns the transform of an entity element from its `Pos`, `Rotate` and `Scale`; a missing
/// attribute is the identity, a malformed one or a degenerate rotation gives `None`.
pub(crate) fn entity_transform<'a>(entity: impl XmlElement<'a>) -> Option<Transform> {
    transform_fields(
        entity.attribute("Pos"),
        entity.attribute("Rotate"),
        entity.attribute("Scale"),
    )
}

pub(crate) fn transform_fields(
    position: Option<&str>,
    rotation: Option<&str>,
    scale: Option<&str>,
) -> Option<Transform> {
    let position = match position {
        None => [0.0; 3],
        Some(text) => numbers::<3>(Some(text))?,
    };
    let rotation = match rotation {
        None => [1.0, 0.0, 0.0, 0.0],
        Some(text) => numbers::<4>(Some(text))?,
    };
    let scale = match scale {
        None => [1.0; 3],
        Some(text) => numbers::<3>(Some(text))?,
    };
    Transform::from_quaternion(rotation, position)?.scaled(scale)
}

fn entities<'a>(
    root: impl XmlElement<'a>,
    chunk: Option<usize>,
) -> Result<SocEntities, SocEntitiesError> {
    let mut result = SocEntities::default();
    let mut seen = 0_usize;
    for entity in root.children() {
        if !entity.name().eq_ignore_ascii_case("Entity") {
            continue;
        }
        seen += 1;
        if seen > MAX_SOC_ENTITIES {
            return Err(SocEntitiesError::TooMany {
                what: "entities",
                limit: MAX_SOC_ENTITIES,
            });
        }
        read_entity(entity, chunk, &mut result)?;
    }
    Ok(result)
}

fn read_entity<'a>(
    entity: impl XmlElement<'a>,
    chunk: Option<usize>,
    out: &mut SocEntities,
) -> Result<(), SocEntitiesError> {
    let name = entity.attribute("Name").unwrap_or_default();
    let before = (out.props.len(), out.lights.len(), out.invalid);
    let transform = entity_transform(entity);
    let class = entity.attribute("EntityClass").unwrap_or_default();
    let geometry = read_geometry(entity);
    let own = geometry
        .as_ref()
        .and_then(|found| found.path.map(|path| (path, found)));
    if let Some((path, found)) = own {
        push_prop(out, name, path, found, transform)?;
    }
    let classed = match entity.attribute("EntityClassGUID") {
        Some(guid) if own.is_none() && !class.eq_ignore_ascii_case("Light") => {
            out.classed.push(SocClassEntity {
                name: name.to_owned(),
                class: guid.trim().to_owned(),
                transform,
                material: geometry
                    .as_ref()
                    .and_then(|found| found.material.map(str::to_owned)),
                palette: geometry.and_then(|found| found.palette),
            });
            true
        }
        _ => false,
    };
    read_lights(entity, (name, class), transform, chunk, out)?;
    if !classed && (out.props.len(), out.lights.len(), out.invalid) == before {
        out.other += 1;
    }
    Ok(())
}

fn push_prop(
    out: &mut SocEntities,
    name: &str,
    path: &str,
    geometry: &EntityGeometry<'_>,
    transform: Option<Transform>,
) -> Result<(), SocEntitiesError> {
    let Some(transform) = transform else {
        out.invalid += 1;
        return Ok(());
    };
    if out.props.len() >= MAX_SOC_PLACED {
        return Err(SocEntitiesError::TooMany {
            what: "props",
            limit: MAX_SOC_PLACED,
        });
    }
    out.props.push(SocProp {
        name: name.to_owned(),
        model: path.to_owned(),
        transform,
        material: geometry.material.map(str::to_owned),
        palette: geometry.palette.clone(),
    });
    Ok(())
}
