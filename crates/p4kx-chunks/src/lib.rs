// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reads the `CryEngine` chunk container that models, skeletons and animations are stored in:
//! the newer `#ivo` form (version `0x900`) and the older `CrCh` form (version `0x746`).
//!
//! [`ChunkFile::parse`] reads the chunk table; each [`Chunk`] carries its [`ChunkType`], version,
//! offset, size and [`ByteOrder`], and [`ChunkFile::chunk_data`] returns its bytes. Interpreting
//! the chunks is left to the model crate.

mod chunk;
mod chunk_directory;
mod container;
mod error;
mod file;
mod known;
mod types;

pub use chunk::{Chunk, ChunkType};
pub use chunk_directory::{ChunkDirectory, ChunkDirectoryError};
pub use container::{ByteOrder, Container};
pub use error::ChunkError;
pub use file::{ChunkFile, ChunkTableRow};
pub use known::KnownChunkType;
pub use types::{crch, ivo};
