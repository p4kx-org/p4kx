// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::SystemTime;

use p4kx_channels::{PublicPatches, channel_name};
use p4kx_extraction::{Archive, utc_rfc3339};

use crate::archive_key_use::ArchiveKeyUse;
use crate::contract::{BuildSource, Source};

pub(crate) fn source<'a>(
    archive: &'a Archive,
    exported_at: SystemTime,
    patches: Option<&'a PublicPatches>,
    key_use: ArchiveKeyUse,
) -> Source<'a> {
    Source {
        channel: channel_name(archive.path()),
        build: archive.build().map(|build| BuildSource {
            branch: build.branch(),
            version: build.version(),
            change_number: build.change_number(),
            build_id: build.build_id(),
            public_patch: patches.and_then(|patches| patches.of(build)),
        }),
        archive_size: archive.file_len(),
        archive_modified: archive.file_modified().map(utc_rfc3339),
        archive_key: key_use.state(),
        encrypted_entries_unread: key_use.entries_unread(),
        tool_version: env!("CARGO_PKG_VERSION"),
        exported_at: utc_rfc3339(exported_at),
    }
}
