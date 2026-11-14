// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::{AtomicU64, Ordering};

use p4kx_extraction::model::{ChunkUsage, MaterialStatus, Model, ModelChunk, ModelFile};

/// The most chunks the model info lists by name; the counts cover all of them.
pub const MAX_LISTED_CHUNKS: usize = 200;

/// The most animations an export summary lists by name; the count covers all of them.
pub const MAX_LISTED_ANIMATIONS: usize = 200;

static SERIAL: AtomicU64 = AtomicU64::new(1);

/// One level of detail of a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LodInfo {
    /// The level, 0 for the most detailed.
    pub level: u32,
    /// The number of vertices of all its meshes.
    pub vertices: usize,
    /// The number of triangles of all its meshes.
    pub triangles: usize,
    /// The number of its meshes.
    pub meshes: usize,
}

/// How many chunks of the model's files the model read, skipped or did not know.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChunkCounts {
    /// All chunks.
    pub total: usize,
    /// The chunks the model was read from.
    pub read: usize,
    /// The chunks of a known type the model does not read.
    pub skipped: usize,
    /// The chunks of an unknown type.
    pub unknown: usize,
}

/// The file of a model that holds a chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkFile {
    /// The model file itself.
    Model,
    /// The geometry companion of level 0.
    Geometry,
    /// The geometry companion of a level of detail from 1 on.
    LodGeometry(u32),
    /// The model file of a level of detail from 1 on, read for its skeleton.
    LodModel(u32),
}

/// A chunk the model did not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkNote {
    /// The file holding the chunk.
    pub file: ChunkFile,
    /// The position in the file's chunk table.
    pub index: usize,
    /// The name of the chunk type, or `None` when it is unknown.
    pub name: Option<&'static str>,
    /// The type ID in hexadecimal.
    pub type_id: String,
    /// Whether the type is unknown rather than known and not read.
    pub unknown: bool,
}

/// What reading the model's material library came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterialsInfo {
    /// The model names no material library.
    Unnamed,
    /// The library does not exist.
    Missing,
    /// The library cannot be read, for the reason given.
    Unreadable(String),
    /// The library was read.
    Loaded {
        /// The number of sub-materials.
        materials: usize,
        /// Whether the file is one material that serves every material ID.
        single: bool,
    },
}

/// How many animations the `.caf` and `.dba` files next to a model and in the folder at the same
/// place below `Data\Animations` hold that an export writes by default, additive ones left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationCount {
    /// The files are being read.
    Reading,
    /// The files were read and hold this many animations.
    Counted(usize),
    /// The files are more than the preview reads, so they were not read.
    TooMany,
    /// The files could not be read.
    Unread,
}

/// The facts of a model file the Info tab lists: chunks, meshes, levels of detail, materials and
/// bones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    serial: u64,
    /// The file name of the model.
    pub name: String,
    /// The file name of the geometry companion, or `None` when the model file holds the meshes.
    pub geometry: Option<String>,
    /// The number of nodes.
    pub nodes: usize,
    /// The counts of the chunks.
    pub chunks: ChunkCounts,
    /// The chunks that were skipped or are unknown, at most [`MAX_LISTED_CHUNKS`].
    pub unread: Vec<ChunkNote>,
    /// The levels of detail, the most detailed first.
    pub lods: Vec<LodInfo>,
    /// The material library the model names, below `Data` without `.mtl`.
    pub library: Option<String>,
    /// What reading the library came to.
    pub materials: MaterialsInfo,
    /// The material IDs the submeshes use, ascending.
    pub material_ids: Vec<u16>,
    /// The number of bones of the skeleton.
    pub bones: usize,
    /// How many animations an export writes by default.
    pub animations: AnimationCount,
}

impl ModelInfo {
    /// Collects the facts of `model`.
    #[must_use]
    pub fn of(model: &Model) -> Self {
        let mut chunks = ChunkCounts::default();
        let mut unread = Vec::new();
        for chunk in &model.chunks {
            chunks.total += 1;
            match chunk.usage {
                ChunkUsage::Read => chunks.read += 1,
                ChunkUsage::Skipped => chunks.skipped += 1,
                ChunkUsage::Unknown => chunks.unknown += 1,
            }
            if chunk.usage != ChunkUsage::Read && unread.len() < MAX_LISTED_CHUNKS {
                unread.push(note(chunk));
            }
        }
        Self {
            serial: SERIAL.fetch_add(1, Ordering::Relaxed),
            name: printable(&model.name),
            geometry: model.geometry.as_deref().map(printable),
            nodes: model.nodes.len(),
            chunks,
            unread,
            lods: model
                .lods
                .iter()
                .map(|lod| LodInfo {
                    level: lod.level,
                    vertices: lod.vertex_count(),
                    triangles: lod.triangle_count(),
                    meshes: lod.meshes.len(),
                })
                .collect(),
            library: model.material_library.as_deref().map(printable),
            materials: materials(&model.materials),
            material_ids: model.material_ids(),
            bones: model.bones.len(),
            animations: AnimationCount::Unread,
        }
    }

    /// Sets the animation count; the info is a new one then, with its own [`Self::serial`].
    #[must_use]
    pub fn with_animations(mut self, animations: AnimationCount) -> Self {
        self.animations = animations;
        self.serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        self
    }

    /// Returns the number that tells this info apart from every other one of the process.
    #[must_use]
    pub const fn serial(&self) -> u64 {
        self.serial
    }

    /// Returns the number of meshes of the most detailed level.
    #[must_use]
    pub fn meshes(&self) -> usize {
        self.lods.first().map_or(0, |lod| lod.meshes)
    }
}

/// Returns `text` with every control and bidirectional override character replaced by the
/// replacement character; names and reasons from an Archive are untrusted.
#[must_use]
pub(crate) fn printable(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() || matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}') {
                char::REPLACEMENT_CHARACTER
            } else {
                c
            }
        })
        .collect()
}

fn note(chunk: &ModelChunk) -> ChunkNote {
    ChunkNote {
        file: match chunk.file {
            ModelFile::LodGeometry(level) => ChunkFile::LodGeometry(level),
            ModelFile::LodModel(level) => ChunkFile::LodModel(level),
            ModelFile::Geometry => ChunkFile::Geometry,
            _ => ChunkFile::Model,
        },
        index: chunk.index,
        name: chunk.chunk_type.name(),
        type_id: chunk.chunk_type.hex_id(),
        unknown: chunk.usage == ChunkUsage::Unknown,
    }
}

fn materials(status: &MaterialStatus) -> MaterialsInfo {
    match status {
        MaterialStatus::Loaded(library) => MaterialsInfo::Loaded {
            materials: library.materials.len(),
            single: library.single,
        },
        MaterialStatus::Missing { .. } => MaterialsInfo::Missing,
        MaterialStatus::Unreadable { reason, .. } => MaterialsInfo::Unreadable(printable(reason)),
        _ => MaterialsInfo::Unnamed,
    }
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::chunks::ChunkFile as SourceChunkFile;
    use p4kx_extraction::model::{CompanionError, CompanionPath, Model};
    use p4kx_testkit::TestModel;

    use super::{ChunkFile, ModelInfo, printable};

    #[test]
    fn control_and_bidirectional_characters_of_an_untrusted_name_are_replaced() {
        assert_eq!(
            printable("a\u{7}b\nc\u{202E}d"),
            "a\u{FFFD}b\u{FFFD}c\u{FFFD}d",
            "replaced"
        );
        assert_eq!(printable("objects/ship"), "objects/ship", "plain stays");
    }

    #[test]
    fn a_level_model_chunk_is_not_attributed_to_the_root_file()
    -> Result<(), Box<dyn std::error::Error>> {
        const UNKNOWN_CHUNK: u32 = 0xDEAD_BEEF;
        let fixture = TestModel::two_bone_skin()
            .with_lod(TestModel::two_bone_skin().with_chunk(UNKNOWN_CHUNK, [7, 8, 9]));
        let source = SourceChunkFile::parse(fixture.cgf())?;
        let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
            Ok(match path {
                CompanionPath::Sibling("skin.skinm") => Some(fixture.cgfm()),
                CompanionPath::Sibling("skin_lod1.skinm") => fixture.lod_cgfm(1),
                CompanionPath::Sibling("skin_lod1.skin") => fixture.lod_cgf(1),
                _ => None,
            })
        };
        let model = Model::read("skin.skin", &source, &mut companions)?;
        let info = ModelInfo::of(&model);
        let note = info
            .unread
            .iter()
            .find(|note| note.type_id == "0xDEADBEEF")
            .ok_or("level model chunk note")?;
        assert_eq!(
            note.file,
            ChunkFile::LodModel(1),
            "actual level-file ownership survives the GUI projection"
        );
        assert!(note.unknown, "unknown source disposition is retained");
        assert_eq!(info.lods.len(), 2, "existing model levels are unchanged");
        Ok(())
    }
}
