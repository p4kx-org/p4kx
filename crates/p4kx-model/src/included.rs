// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::ChunkFile;
use p4kx_foundation::ParseError;
use thiserror::Error;

use crate::Transform;
use crate::included_object_row::{self, IncludedObjectKind, IncludedPlacement};
use crate::included_objects_chunk::{self, IncludedObjectTables};

/// The most model paths one `.soc` file lists.
pub const MAX_INCLUDED_MODELS: usize = 1 << 16;

/// The most placed objects one `.soc` file lists.
pub const MAX_INCLUDED_OBJECTS: usize = 1 << 20;

/// A failure to read the included objects of a `.soc` file.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum IncludedObjectsError {
    /// The file is no chunk file this crate reads.
    #[error("the file is not a readable chunk file")]
    Chunks(#[from] p4kx_chunks::ChunkError),
    /// The chunk ends before a value it declares.
    #[error("the included objects chunk is truncated")]
    Truncated(#[from] ParseError),
    /// The chunk is flagged big-endian, which is not read.
    #[error("the included objects chunk is big-endian")]
    BigEndian,
    /// The chunk lists more of something than is read.
    #[error("the included objects chunk lists more than {limit} {what}")]
    TooMany {
        /// What there is too much of.
        what: &'static str,
        /// The most read.
        limit: usize,
    },
    /// The object table does not fit the chunk.
    #[error("the object table of {declared} bytes does not fit the {available} bytes left")]
    ObjectTable {
        /// The length the chunk declares.
        declared: u64,
        /// The bytes left in the chunk.
        available: u64,
    },
    /// The object table has a record layout that is not read.
    #[error("the object table has the record layout {layout}, which is not read")]
    RecordLayout {
        /// The layout word at the head of the table.
        layout: u32,
    },
}

/// One model placed by a `.soc` file.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct IncludedObject {
    /// The position of the model's path in [`IncludedObjects::models`].
    pub model: usize,
    /// The transform from the model's space to the container's, in `CryEngine` coordinates, Z up
    /// and in metres.
    pub transform: Transform,
    /// The position of the tint palette in [`IncludedObjects::palettes`], if the object has one.
    pub palette: Option<usize>,
}

/// The models a `.soc` file places: the `IncludedObjects` chunk (`CrCh` type `0x0010`) with the
/// model paths and tint palettes it lists and one [`IncludedObject`] per placement.
///
/// The record length depends on the layout word of the object table. Records of kind 16 are
/// skipped and counted; records of a kind whose length is not known end the list. Objects that
/// name no listed model or have a transform that is not finite or not
/// [plausible](Transform::is_plausible_placement) are left out and counted.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct IncludedObjects {
    /// The model paths, as the file spells them: below `Data`, usually `.cgf` files.
    pub models: Vec<String>,
    /// The Record paths of the tint palettes.
    pub palettes: Vec<String>,
    /// The placed models in file order.
    pub objects: Vec<IncludedObject>,
    /// The number of objects left out because they name no listed model, have no plausible
    /// transform or are cut off.
    pub invalid: usize,
    /// The number of records of kind 16, which place no model and are skipped.
    pub skipped: usize,
    /// The kind of the first record that is neither a placed model nor skipped, whose length is
    /// not known and which therefore ends the list.
    pub unsupported: Option<u32>,
}

impl IncludedObjects {
    /// Reads the `IncludedObjects` chunks of the `CrCh` chunk file `bytes`, a `.soc` file, and
    /// joins them; returns `None` when it holds none.
    ///
    /// # Errors
    ///
    /// Returns [`IncludedObjectsError`] when the file is no chunk file or a chunk is truncated or
    /// declares more than is read.
    pub fn read(bytes: &[u8]) -> Result<Option<Self>, IncludedObjectsError> {
        let file = ChunkFile::parse(bytes)?;
        let mut found: Option<Self> = None;
        for chunk in included_objects_chunk::chunks(&file) {
            let tables = chunk.read_tables(Some(MAX_INCLUDED_MODELS))?;
            let chunk = preview(&tables)?;
            found = Some(match found {
                Some(first) => first.joined(chunk),
                None => chunk,
            });
        }
        Ok(found)
    }

    fn joined(mut self, other: Self) -> Self {
        let models = self.models.len();
        let palettes = self.palettes.len();
        self.models.extend(other.models);
        self.palettes.extend(other.palettes);
        self.objects
            .extend(other.objects.into_iter().map(|object| IncludedObject {
                model: object.model + models,
                palette: object.palette.map(|palette| palette + palettes),
                ..object
            }));
        self.invalid += other.invalid;
        self.skipped += other.skipped;
        self.unsupported = self.unsupported.or(other.unsupported);
        self
    }
}

fn preview(tables: &IncludedObjectTables<'_>) -> Result<IncludedObjects, IncludedObjectsError> {
    let declared = u64::from(tables.object_len_declared());
    let available = tables.byte_len() as u64;
    if declared > available {
        return Err(IncludedObjectsError::ObjectTable {
            declared,
            available,
        });
    }
    let models = tables
        .models()
        .map(|slot| String::from_utf8_lossy(slot.path_bytes()).trim().to_owned())
        .collect();
    let palettes = tables
        .palettes()
        .map(|slot| String::from_utf8_lossy(slot.path_bytes()).trim().to_owned())
        .collect();
    let mut result = IncludedObjects {
        models,
        palettes,
        ..IncludedObjects::default()
    };
    if tables.byte_len() == 0 {
        return Ok(result);
    }
    let layout = tables.layout();
    let stride = included_object_row::placement_len(layout)
        .ok_or(IncludedObjectsError::RecordLayout { layout })?;
    check(
        tables.byte_len() / stride.min(included_object_row::KIND_16_LEN),
        MAX_INCLUDED_OBJECTS,
        "objects",
    )?;
    for row in tables.rows() {
        match row.kind() {
            IncludedObjectKind::Placement(placement) => {
                if let Some(object) =
                    preview_object(placement, result.models.len(), result.palettes.len())
                {
                    result.objects.push(object);
                } else {
                    result.invalid += 1;
                }
            }
            IncludedObjectKind::Kind16 => result.skipped += 1,
            IncludedObjectKind::UnmeasuredKind { kind }
                if row.bytes().len() >= stride.min(included_object_row::KIND_16_LEN) =>
            {
                result.unsupported = Some(kind);
            }
            IncludedObjectKind::Truncated { .. } | IncludedObjectKind::UnmeasuredKind { .. } => {
                result.invalid += 1;
            }
            IncludedObjectKind::UnmeasuredLayout => {
                return Err(IncludedObjectsError::RecordLayout { layout });
            }
        }
    }
    Ok(result)
}

pub(crate) const fn check(
    count: usize,
    limit: usize,
    what: &'static str,
) -> Result<(), IncludedObjectsError> {
    if count > limit {
        Err(IncludedObjectsError::TooMany { what, limit })
    } else {
        Ok(())
    }
}

fn preview_object(
    placement: IncludedPlacement<'_>,
    models: usize,
    palettes: usize,
) -> Option<IncludedObject> {
    let model = usize::from(placement.model_index());
    if model >= models {
        return None;
    }
    let rows = placement
        .transform_words()
        .map(|row| row.map(f64::from_bits));
    let transform = Transform::from_rows_f64(rows).filter(Transform::is_plausible_placement)?;
    let palette = usize::from(placement.palette_byte());
    Some(IncludedObject {
        model,
        transform,
        palette: (palette < palettes).then_some(palette),
    })
}
