// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use anyhow::Result;
use p4kx_assembly::{
    ArchiveFiles, BodyVariant, EntityLookupError, EntityNode, Expressions, LoadoutFileError,
    LoadoutFiles, LoadoutResolver, MAX_XML_LOADOUT_LEN, NpcHead, OutfitReport, Piece, Replaced,
};
use serde::Serialize;
use thiserror::Error;

use crate::entity_outfit::{self, OutfitsJson};
use crate::query_table::one_line;

/// A `--loadout` that names no Entity and no XML Loadout file that can be read.
#[derive(Debug, Error)]
#[error("{text:?} is no Entity and no readable XML Loadout: {reason}")]
pub(crate) struct UnreadableLoadout {
    text: String,
    reason: String,
}

/// Where a Loadout given with `--loadout` comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LoadoutSource {
    /// The default Loadout of an Entity Record.
    Entity,
    /// An XML Loadout Entry of the Archive.
    Archive,
    /// An XML Loadout file on disk.
    File,
}

impl LoadoutSource {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::Archive => "archive",
            Self::File => "file",
        }
    }
}

/// The XML Loadout files the resolver reads: one file from disk, else the Archive.
pub(crate) struct Files<'a> {
    local: Option<(String, Vec<u8>)>,
    archive: ArchiveFiles<'a>,
}

impl<'a> Files<'a> {
    /// Reads the file the text `--loadout` gives names when it is a file on disk, else only the
    /// Archive.
    pub(crate) fn open(archive: ArchiveFiles<'a>, text: Option<&str>) -> Result<Self> {
        let local = match text.filter(|text| Path::new(text).is_file()) {
            Some(text) => Some((text.to_owned(), read_local(text)?)),
            None => None,
        };
        Ok(Self { local, archive })
    }

    fn local_name(&self) -> Option<&str> {
        self.local.as_ref().map(|(name, _)| name.as_str())
    }
}

impl LoadoutFiles for Files<'_> {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        match &self.local {
            Some((name, bytes)) if path.eq_ignore_ascii_case(name) => Ok(bytes.clone()),
            _ => self.archive.read(path),
        }
    }
}

fn read_local(text: &str) -> Result<Vec<u8>> {
    let unreadable = |reason: String| UnreadableLoadout {
        text: text.to_owned(),
        reason,
    };
    let mut bytes = Vec::new();
    File::open(text)
        .and_then(|file| {
            file.take(MAX_XML_LOADOUT_LEN.saturating_add(1))
                .read_to_end(&mut bytes)
        })
        .map_err(|error| unreadable(error.to_string()))?;
    if u64::try_from(bytes.len()).is_ok_and(|len| len > MAX_XML_LOADOUT_LEN) {
        return Err(unreadable(
            LoadoutFileError::TooLarge {
                size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            }
            .to_string(),
        )
        .into());
    }
    Ok(bytes)
}

/// A resolved Loadout and where it came from.
struct Resolved {
    tree: EntityNode,
    source: LoadoutSource,
    files: Vec<String>,
    outfits: Option<OutfitReport>,
}

/// The Loadout `--loadout` named, as the report shows it: where it came from and the XML Loadout
/// files it is made of, in the order they are laid over each other.
#[derive(Debug, Serialize)]
pub(crate) struct LoadoutJson<'a> {
    pub(crate) name: &'a str,
    pub(crate) source: LoadoutSource,
    pub(crate) class_name: Option<String>,
    pub(crate) files: Vec<String>,
    #[serde(rename = "outfit", skip_serializing_if = "Option::is_none")]
    pub(crate) outfits: Option<OutfitsJson>,
}

/// The head a Loadout gave its character, as the report shows it.
#[derive(Debug, Serialize)]
pub(crate) struct HeadJson {
    pub(crate) class_name: String,
    pub(crate) body: &'static str,
    pub(crate) bespoke_folder: Option<String>,
    pub(crate) parts: usize,
}

/// A piece of the Loadout that a `--piece` took the place of, as the report shows it.
#[derive(Debug, Serialize)]
pub(crate) struct ReplacedJson<'a> {
    pub(crate) name: &'a str,
    pub(crate) by: &'a str,
    pub(crate) item_type: &'a str,
}

impl<'a> From<&'a Replaced> for ReplacedJson<'a> {
    fn from(replaced: &'a Replaced) -> Self {
        Self {
            name: replaced.name(),
            by: replaced.by(),
            item_type: replaced.item_type(),
        }
    }
}

/// The pieces to wear: those of `--loadout`, with the `--piece`s on top.
pub(crate) struct Selection<'a> {
    pub(crate) pieces: Vec<Piece>,
    pub(crate) loadout: Option<LoadoutJson<'a>>,
    pub(crate) replaced: Vec<Replaced>,
    pub(crate) head: Option<NpcHead>,
}

impl Selection<'_> {
    /// Returns the body variant the Loadout's head belongs on.
    pub(crate) fn variant(&self) -> Option<BodyVariant> {
        self.head.as_ref().map(NpcHead::variant)
    }

    /// Returns the report of the Loadout's head.
    pub(crate) fn head_json(&self) -> Option<HeadJson> {
        self.head.as_ref().map(|head| HeadJson {
            class_name: head.class_name().to_owned(),
            body: head.variant().code(),
            bespoke_folder: head.folder().map(str::to_owned),
            parts: head.pieces().len(),
        })
    }
}

impl<'a> Selection<'a> {
    /// Finds the Entities of the `--piece`s and resolves the `--loadout`, if there is one; the
    /// head the Loadout gives its character is written with `expressions`.
    pub(crate) fn gather(
        resolver: &mut LoadoutResolver<'_>,
        files: &Files<'_>,
        (names, loadout): (&[String], Option<&'a str>),
        expressions: &Expressions,
    ) -> Result<Self> {
        let given = names
            .iter()
            .map(|text| piece(resolver, text))
            .collect::<Result<Vec<_>>>()?;
        let Some(text) = loadout else {
            return Ok(Self {
                pieces: given,
                loadout: None,
                replaced: Vec::new(),
                head: None,
            });
        };
        let found = resolve(resolver, files, text)?;
        let (mut pieces, replaced) = resolver.worn_layered(&found.tree, given);
        let head = resolver
            .npc_head(&found.tree)
            .map(|head| head.with_expressions(expressions));
        if let Some(head) = &head {
            pieces.extend(head.pieces().iter().cloned());
        }
        Ok(Self {
            pieces,
            loadout: Some(LoadoutJson {
                name: text,
                source: found.source,
                class_name: (found.source == LoadoutSource::Entity)
                    .then(|| found.tree.class_name().to_owned()),
                files: found.files,
                outfits: found.outfits.as_ref().map(OutfitsJson::from),
            }),
            replaced,
            head,
        })
    }

    /// Writes the lines of the text report that tell where the pieces come from.
    pub(crate) fn print(&self, out: &mut impl Write) -> io::Result<()> {
        if let Some(loadout) = &self.loadout {
            writeln!(
                out,
                "loadout {} ({})",
                one_line(loadout.name),
                loadout.source.code()
            )?;
            for file in &loadout.files {
                writeln!(out, "loadout file {}", one_line(file))?;
            }
            if let Some(outfits) = &loadout.outfits {
                entity_outfit::print(out, outfits)?;
            }
        }
        if let Some(head) = &self.head {
            let own = head.folder().map_or_else(String::new, |folder| {
                format!(", its own skins in {}", one_line(folder))
            });
            writeln!(
                out,
                "head {} on the {} body, {} parts{own}",
                one_line(head.class_name()),
                head.variant().code(),
                head.pieces().len()
            )?;
        }
        for replaced in &self.replaced {
            writeln!(
                out,
                "replaced {} by {} ({})",
                one_line(replaced.name()),
                one_line(replaced.by()),
                one_line(replaced.item_type())
            )?;
        }
        Ok(())
    }
}

/// Finds the Entity `text` names and reads what it offers to be worn; an Entity that cannot be
/// read stays a piece that the report lists.
fn piece(resolver: &mut LoadoutResolver<'_>, text: &str) -> Result<Piece> {
    let record = resolver.entities().find(text)?;
    Ok(match resolver.wearable(&record) {
        Ok(wearable) => Piece::wearable(wearable),
        Err(error) => Piece::failed(text, error.to_string()),
    })
}

/// Resolves what `--loadout` names: a file on disk, else an Entity by class name, GUID or
/// record path, else an XML Loadout Entry of the Archive.
fn resolve(resolver: &mut LoadoutResolver<'_>, files: &Files<'_>, text: &str) -> Result<Resolved> {
    let unreadable = |error: p4kx_assembly::LoadoutError| UnreadableLoadout {
        text: text.to_owned(),
        reason: error.to_string(),
    };
    if let Some(name) = files.local_name() {
        let tree = resolver.resolve_file(name).map_err(unreadable)?;
        return Ok(Resolved {
            tree,
            source: LoadoutSource::File,
            files: vec![name.to_owned()],
            outfits: None,
        });
    }
    match resolver.entities().find(text) {
        Ok(record) => Ok(Resolved {
            tree: resolver.resolve(&record)?,
            source: LoadoutSource::Entity,
            files: resolver.loadout_files(&record),
            outfits: resolver.outfits(&record),
        }),
        Err(EntityLookupError::NotFound { .. }) if text.to_ascii_lowercase().ends_with(".xml") => {
            let tree = resolver.resolve_file(text).map_err(unreadable)?;
            Ok(Resolved {
                tree,
                source: LoadoutSource::Archive,
                files: vec![text.to_owned()],
                outfits: None,
            })
        }
        Err(error) => Err(error.into()),
    }
}
