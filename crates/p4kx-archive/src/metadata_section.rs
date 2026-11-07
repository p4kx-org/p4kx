// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A source metadata section, distinct from payload bytes and exported artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MetadataSection {
    /// Declared directory bytes, including unadmitted folders and uninterpreted trailing bytes.
    Directory,
    /// Classic ZIP fixed end record.
    ZipEnd,
    /// Original classic ZIP comment bytes.
    ZipComment,
    /// Uninterpreted bytes after a complete classic ZIP comment.
    ZipFollowing,
    /// Fixed ZIP64 end record.
    Zip64End,
    /// Declared ZIP64 extensible data after the fixed record.
    Zip64Extension,
    /// ZIP64 locator actually selected by the reader.
    Zip64Locator,
    /// Original v2 base directory records.
    V2BaseDirectory,
    /// Original v2 extension directory records.
    V2ExtensionDirectory,
    /// Original v2 base name-table bytes.
    V2BaseNames,
    /// Original v2 extension name-table bytes.
    V2ExtensionNames,
    /// Complete fixed v2 trailer.
    V2Trailer,
    /// First opaque 32-byte trailer slot, without a verification claim.
    V2OpaqueFirst,
    /// Second opaque 32-byte trailer slot, without an algorithm claim.
    V2OpaqueSecond,
    /// Declared unparsed v2 trie-cache bytes.
    V2Trie,
    /// Original installation progress records.
    Installation,
    /// Original free-range records.
    FreeRanges,
}
