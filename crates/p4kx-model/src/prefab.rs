// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_cryxml::{CryXmlError, Xml};
use thiserror::Error;

use crate::Transform;
use crate::mtl_xml::XmlElement;
use crate::prefab_diagnostic::PrefabParseFailure;
use crate::prefab_guid_remapping::PrefabGuidRemapping;
use crate::prefab_objects;
use crate::prefab_source::{PositionedPrefabLibrary, PrefabLibrarySource, PrefabSource};

/// The most prefabs one library lists.
pub const MAX_LIBRARY_PREFABS: usize = 1 << 16;

/// The most objects one library lists, nested groups counted.
pub const MAX_LIBRARY_OBJECTS: usize = 1 << 21;

/// The deepest nesting of groups in a prefab that is read; deeper objects are left out.
pub const MAX_GROUP_DEPTH: usize = 64;

/// A failure to read a prefab library.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PrefabError {
    /// The file is no readable `CryXml`.
    #[error("the file is not valid CryXml")]
    CryXml(#[from] CryXmlError),
    /// The file is no well-formed text XML.
    #[error("the file is not well-formed XML")]
    Xml(#[from] roxmltree::Error),
    /// The text XML nests its elements too deeply to be read.
    #[error("the file is not readable XML: {0}")]
    TooDeep(#[from] p4kx_cryxml::TextTooDeep),
    /// The root element is no `PrefabsLibrary` and holds no `Prefab`.
    #[error("the root element is {found}, which is no prefab library")]
    NotLibrary {
        /// The root element's name.
        found: String,
    },
    /// The file lists more of something than is read.
    #[error("the file lists more than {limit} {what}")]
    TooMany {
        /// What there is too much of.
        what: &'static str,
        /// The most read.
        limit: usize,
    },
}

/// The prefab an object of type `Prefab` instances.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabRef {
    /// The `PrefabGUID` as the file spells it, empty when it names none.
    pub guid: String,
    /// The `PrefabName`: the library name and the prefab name joined by `.`, e.g.
    /// `pu/derelict/components.visual.flare`; empty when it names none.
    pub name: String,
    /// Authored `GUIDRemapping` blocks and their pairs in file order.
    pub guid_remapping_blocks: Vec<Vec<PrefabGuidRemapping>>,
}

/// An object of a prefab, placed in the prefab's space.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PrefabObject {
    /// The object's `Name`.
    pub name: String,
    /// The object's `Type`, e.g. `Entity`, `Brush`, `GeomEntity` or `Prefab`.
    pub kind: String,
    /// The object's `Id`, an editor GUID as the file spells it.
    pub id: String,
    /// The `EntityClass`, empty when the object has none.
    pub class: String,
    /// The geometry file the object names as the file spells it, below `Data`: the `Prefab` of a
    /// brush, the `Geometry` of a geometry entity or the geometry resource of an entity.
    pub model: Option<String>,
    /// The transform from the object's space to the prefab's, `CryEngine` coordinates, Z up and
    /// in metres, with the transforms of the groups around the object already applied.
    pub transform: Transform,
    /// The prefab the object instances, for an object of type `Prefab`.
    pub nested: Option<PrefabRef>,
    /// Whether the object is attached to another object (`Parent` other than its group, or an
    /// `AttachmentTarget`), in which case its transform is relative to that attachment and not
    /// to the prefab.
    pub attached: bool,
}

/// A prefab: the objects of a reusable piece of a level.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Prefab {
    /// The prefab's `Name` within its library.
    pub name: String,
    /// The prefab's `Id`, an editor GUID as the file spells it.
    pub id: String,
    /// The `Library` the prefab names.
    pub library: String,
    /// The objects in file order, groups flattened into them and left out themselves.
    pub objects: Vec<PrefabObject>,
    /// The number of objects left out because their transform is not usable or they sit too
    /// deep in groups.
    pub invalid: usize,
}

/// A prefab library: an XML file below `DataPrefabs` whose root, a `PrefabsLibrary` or an element
/// named after the library, holds the prefabs.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct PrefabLibrary {
    /// The root's `Name`, or the root element's own name when it has none.
    pub name: String,
    /// The prefabs in file order.
    pub prefabs: Vec<Prefab>,
}

impl PrefabLibrary {
    /// Reads the library in `bytes`, text XML or `CryXml`.
    ///
    /// # Errors
    ///
    /// Returns [`PrefabError`] when the file is neither, its root is no `PrefabsLibrary` or it
    /// lists more prefabs or objects than are read.
    pub fn parse(bytes: &[u8]) -> Result<Self, PrefabError> {
        parse_library(bytes, PrefabParseMode::ValuesOnly)
            .map(|parsed| parsed.library)
            .map_err(PrefabParseFailure::into_error)
    }

    /// Reads the library and the positions of its original fields.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::parse`].
    pub fn parse_positioned(bytes: &[u8]) -> Result<PositionedPrefabLibrary, PrefabError> {
        parse_library(bytes, PrefabParseMode::Positions).map_err(PrefabParseFailure::into_error)
    }

    /// Reads the library with original field and flattened object coordinates.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::parse`].
    pub fn parse_positioned_objects(bytes: &[u8]) -> Result<PositionedPrefabLibrary, PrefabError> {
        parse_library(bytes, PrefabParseMode::Objects).map_err(PrefabParseFailure::into_error)
    }

    /// Reads a library and retains original values used by a failed-read diagnostic.
    ///
    /// # Errors
    ///
    /// Returns the original Prefab parse error and any retained diagnostic source.
    pub fn parse_observed(
        bytes: &[u8],
        mode: PrefabParseMode,
    ) -> Result<PositionedPrefabLibrary, PrefabParseFailure> {
        parse_library(bytes, mode)
    }

    /// Returns the first prefab named `name`, ignoring case.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&Prefab> {
        self.prefabs
            .iter()
            .find(|prefab| prefab.name.eq_ignore_ascii_case(name))
    }
}

/// Selects the original position data kept for a successful Prefab parse.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum PrefabParseMode {
    /// Keep the parsed values.
    ValuesOnly,
    /// Keep library and Prefab row positions.
    Positions,
    /// Keep those positions and the surviving objects' original fields.
    Objects,
}

impl PrefabParseMode {
    const fn omit_if_repaired(self, repaired: bool) -> Self {
        if repaired { Self::ValuesOnly } else { self }
    }
}

fn parse_library(
    bytes: &[u8],
    capture: PrefabParseMode,
) -> Result<PositionedPrefabLibrary, PrefabParseFailure> {
    match p4kx_cryxml::read(bytes).map_err(PrefabError::from)? {
        Xml::CryXml(document) => library(document.root(), capture, document.is_lossy()),
        Xml::Text(text) => {
            let text = String::from_utf8_lossy(text);
            p4kx_cryxml::check_text_depth(&text).map_err(PrefabError::from)?;
            let document = roxmltree::Document::parse(&text).map_err(PrefabError::from)?;
            library(
                document.root_element(),
                capture,
                matches!(text, Cow::Owned(_)),
            )
        }
    }
}

fn library<'a>(
    root: impl XmlElement<'a>,
    capture: PrefabParseMode,
    repaired: bool,
) -> Result<PositionedPrefabLibrary, PrefabParseFailure> {
    let is_library = root.name().eq_ignore_ascii_case("PrefabsLibrary")
        || root
            .children()
            .any(|element| element.name().eq_ignore_ascii_case("Prefab"));
    if !is_library {
        return Err(PrefabParseFailure::root_name(
            root.name(),
            (!repaired).then(|| root.position()),
        ));
    }
    let capture = capture.omit_if_repaired(repaired);
    let authored_name = root
        .attribute("Name")
        .map(str::trim)
        .filter(|name| !name.is_empty());
    let mut library = PrefabLibrary {
        name: authored_name.unwrap_or_else(|| root.name()).to_owned(),
        prefabs: Vec::new(),
    };
    let mut source = match capture {
        PrefabParseMode::Positions | PrefabParseMode::Objects => {
            PrefabLibrarySource::new(root, authored_name)
        }
        PrefabParseMode::ValuesOnly => None,
    };
    let mut total_objects = 0_usize;
    for element in root.children() {
        if !element.name().eq_ignore_ascii_case("Prefab") {
            continue;
        }
        if library.prefabs.len() >= MAX_LIBRARY_PREFABS {
            return Err(PrefabError::TooMany {
                what: "prefabs",
                limit: MAX_LIBRARY_PREFABS,
            }
            .into());
        }
        let objects = prefab_objects::child(element, "Objects");
        let mut row = source.as_ref().map(|_| PrefabSource::new(element, objects));
        let object_sources = match capture {
            PrefabParseMode::Objects => row
                .as_mut()
                .map(|row| row.object_sources.insert(Vec::new())),
            PrefabParseMode::ValuesOnly | PrefabParseMode::Positions => None,
        };
        library.prefabs.push(prefab_objects::read(
            element,
            objects,
            &mut total_objects,
            object_sources,
        )?);
        if let (Some(source), Some(row)) = (&mut source, row) {
            source.prefabs.push(row);
        }
    }
    Ok(PositionedPrefabLibrary { library, source })
}
