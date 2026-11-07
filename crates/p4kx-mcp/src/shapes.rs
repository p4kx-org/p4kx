// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_channels::Channel;
use p4kx_extraction::{Build, Entry, EntryMetadataInfo};
use schemars::JsonSchema;
use serde::Serialize;

/// The detected Channels.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ChannelsResult {
    /// The number of detected Channels.
    pub(crate) total: usize,
    /// The Channels on this page.
    pub(crate) channels: Vec<ChannelShape>,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
}

/// One detected Channel.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ChannelShape {
    /// The Channel name, e.g. `LIVE`; it opens the Channel's Archive in every tool.
    pub(crate) name: String,
    /// The path of the Archive.
    pub(crate) archive: String,
    /// Where it was found: `default`, `launcher` or `manual`.
    pub(crate) source: String,
    /// The Build, absent when the manifest cannot be read.
    pub(crate) build: Option<BuildShape>,
}

impl From<&Channel> for ChannelShape {
    fn from(channel: &Channel) -> Self {
        Self {
            name: channel.name().to_owned(),
            archive: channel.archive().display().to_string(),
            source: channel.source().to_string(),
            build: channel.build().map(BuildShape::from),
        }
    }
}

/// The game version an Archive belongs to.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct BuildShape {
    /// The branch, e.g. `sc-alpha-4.3.1`.
    pub(crate) branch: String,
    /// The version, e.g. `4.3.1-live.10000001`.
    pub(crate) version: String,
    /// The change number.
    pub(crate) change_number: String,
    /// The `BuildId`.
    pub(crate) build_id: String,
    /// The public patch version, e.g. `4.7.2`, from the RSI Launcher log; `null` when the log has
    /// no line for the change number.
    pub(crate) public_patch: Option<String>,
}

impl From<&Build> for BuildShape {
    fn from(build: &Build) -> Self {
        Self {
            branch: build.branch().to_owned(),
            version: build.version().to_owned(),
            change_number: build.change_number().to_owned(),
            build_id: build.build_id().to_owned(),
            public_patch: build.public_patch().map(str::to_owned),
        }
    }
}

/// The summary of one Archive.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ArchiveInfo {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The Channel name, the folder holding the Archive.
    pub(crate) channel: Option<String>,
    /// The Build, absent when the manifest cannot be read.
    pub(crate) build: Option<BuildShape>,
    /// The number of Entries.
    pub(crate) entry_count: usize,
    /// The number of encrypted Entries.
    pub(crate) encrypted_count: usize,
    /// The number of defects found in the Entries' directory records.
    pub(crate) issue_count: usize,
    /// The first of those defects, at most 50.
    pub(crate) issues: Vec<IssueShape>,
    /// The sum of the stored Entry sizes in bytes.
    pub(crate) compressed_size: u64,
    /// The sum of the Entry sizes in bytes.
    pub(crate) size: u64,
}

/// A defect of one Entry that did not keep the Archive from opening.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct IssueShape {
    /// The Entry path in the Archive, with invalid UTF-8 bytes replaced.
    pub(crate) path: String,
    /// What is wrong with the Entry.
    pub(crate) issue: String,
}

/// One Entry.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct EntryShape {
    /// The Entry path in the Archive.
    pub(crate) path: String,
    /// The compression method: `store`, `deflate`, `zstd` or `method-<number>`.
    pub(crate) compression: String,
    /// Whether the Entry is encrypted.
    pub(crate) encrypted: bool,
    /// The stored size in bytes.
    pub(crate) compressed_size: u64,
    /// The size in bytes.
    pub(crate) size: u64,
    /// Directory facts owned by this exact physical Entry, including its admitted index.
    pub(crate) metadata: EntryMetadataInfo,
}

impl From<&Entry> for EntryShape {
    fn from(entry: &Entry) -> Self {
        Self::at(entry, entry.path().to_owned())
    }
}

impl EntryShape {
    /// Describes `entry` under the path `path`, which a tool takes to read it.
    pub(crate) fn at(entry: &Entry, path: String) -> Self {
        Self {
            path,
            compression: entry.compression().to_string(),
            encrypted: entry.is_encrypted(),
            compressed_size: entry.compressed_size(),
            size: entry.uncompressed_size(),
            metadata: EntryMetadataInfo::of(entry),
        }
    }
}

/// The content of one folder of an Archive: its subfolders first, then its Entries.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct FolderResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The folder listed; empty for the top level. Inside a container it begins with the
    /// containers, `outer.socpak::folder`.
    pub(crate) folder: String,
    /// The containers the folder lies in, `outer.socpak::inner.socpak`; absent for the Archive.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) container: Option<String>,
    /// The number of subfolders and Entries in the folder.
    pub(crate) total: usize,
    /// The subfolders and Entries on this page.
    pub(crate) items: Vec<FolderItem>,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
}

/// A subfolder or an Entry of a folder.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum FolderItem {
    /// A subfolder.
    Folder {
        /// The folder path in the Archive; pass it as `folder` to list it.
        path: String,
        /// The number of Entries below it, at any depth.
        entry_count: usize,
    },
    /// An Entry directly in the folder.
    Entry(EntryShape),
}

/// The Entries a Filter matched.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct SearchResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The Filter.
    pub(crate) filter: String,
    /// The containers searched, `outer.socpak::inner.socpak`; absent for the Archive.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) container: Option<String>,
    /// The number of matching Entries.
    pub(crate) total: usize,
    /// The matching Entries on this page, in Archive order.
    pub(crate) entries: Vec<EntryShape>,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
}

/// A range of the text of an Entry; offsets and lengths count bytes of its UTF-8 form.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct TextResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The Entry path in the Archive.
    pub(crate) path: String,
    /// How the Entry was decoded: `utf-8`, `utf-16le`, `utf-16be`, or `cryxml` converted to XML.
    pub(crate) encoding: &'static str,
    /// The size of the whole text in UTF-8 bytes.
    pub(crate) size: u64,
    /// The offset of `text` in the whole text; an offset inside a character moves to the next one.
    pub(crate) offset: u64,
    /// The length of `text` in UTF-8 bytes.
    pub(crate) length: usize,
    /// The text read.
    pub(crate) text: String,
    /// Whether more text follows.
    pub(crate) more: bool,
    /// The offset to continue at; absent at the end.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_offset: Option<u64>,
    /// Whether `max_bytes` was raised or lowered to the allowed range.
    pub(crate) clamped: bool,
    /// The `max_bytes` used.
    pub(crate) max_bytes: usize,
}

/// A range of the data of an Entry, base64 encoded.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct BytesResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The Entry path in the Archive.
    pub(crate) path: String,
    /// The size of the Entry data in bytes.
    pub(crate) size: u64,
    /// The offset of the first byte in the Entry data.
    pub(crate) offset: u64,
    /// The number of bytes read.
    pub(crate) length: usize,
    /// The encoding of `data`: always `base64`, the standard alphabet with padding.
    pub(crate) encoding: &'static str,
    /// The bytes read.
    pub(crate) data: String,
    /// Whether more bytes follow.
    pub(crate) more: bool,
    /// The offset to continue at; absent at the end.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_offset: Option<u64>,
    /// Whether `max_bytes` was raised or lowered to the allowed range.
    pub(crate) clamped: bool,
    /// The `max_bytes` used.
    pub(crate) max_bytes: usize,
}
