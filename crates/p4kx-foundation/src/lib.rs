// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Shared building blocks of the P4KX format crates: a bounds-checked little-endian
//! reader over byte slices, the parse error it reports, path matching with the rules of a Filter,
//! the Archive key type and IV, and the tolerant reading of numbers in text data.

mod archive_key;
mod byte_reader;
mod parse_error;
mod path_pattern;
#[cfg(feature = "platform-folders")]
mod platform_folders;
mod text_number;

pub use archive_key::{ARCHIVE_IV, ArchiveKey, ArchiveKeyError};
pub use byte_reader::ByteReader;
pub use parse_error::ParseError;
pub use path_pattern::PathPattern;
#[cfg(feature = "platform-folders")]
pub use platform_folders::{FOLDERS_ROOT_VARIABLE, FolderKind, XdgEnvironment, base_folder};
pub use text_number::{TextNumber, is_format_character, read_text_number};
