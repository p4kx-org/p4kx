// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice::Iter;

use p4kx_extraction::sound::wwise_path;
use p4kx_extraction::{Archive, Entry};
use p4kx_media::{ContainerIdentity, Identity, MediaRecord, Status};
use serde_json::json;

use super::super::shader_accounting::require;
use super::TestResult;

const CANDIDATE: &str = "audio-loose-wem-candidate";
const INVENTORY: &str = "audio-loose-wem-inventory";

pub(super) struct RootInventory<'a> {
    archive: &'a Archive,
    entries: Iter<'a, Entry>,
    selected: usize,
    observed: usize,
    closed: bool,
}

impl<'a> RootInventory<'a> {
    pub(super) fn new(archive: &'a Archive, selected: usize) -> Self {
        Self {
            archive,
            entries: archive.entries().iter(),
            selected,
            observed: 0,
            closed: false,
        }
    }

    pub(super) fn accepts(&mut self, row: &MediaRecord) -> TestResult<bool> {
        match row.subresource.namespace.as_str() {
            CANDIDATE | INVENTORY => {
                require(
                    !self.closed
                        && matches!(row.source.identity, Identity::Root)
                        && row.source.raw.is_none()
                        && row.status == Status::Complete,
                    "root directory-only audio occurrence",
                )?;
            }
            _ => return Ok(false),
        }
        if row.subresource.namespace == INVENTORY {
            self.close(row)?;
        } else {
            self.candidate(row)?;
        }
        Ok(true)
    }

    pub(super) fn finish(self) -> TestResult {
        require(self.closed, "complete root loose WEM inventory closure")
    }

    fn close(&mut self, row: &MediaRecord) -> TestResult {
        require(
            self.entries
                .all(|entry| wwise_path::loose_name(entry.path()).is_none()),
            "all physical loose WEM candidates are present",
        )?;
        require(
            row.subresource.indices.is_empty()
                && row.data.pointer("/payload/candidates_complete") == Some(&json!(true))
                && row.data.pointer("/payload/observed_candidates")
                    == Some(&json!(self.observed.to_string())),
            "root loose WEM inventory closure agrees with the source",
        )?;
        self.closed = true;
        Ok(())
    }

    fn candidate(&mut self, row: &MediaRecord) -> TestResult {
        let (entry, name) = self
            .entries
            .find_map(|entry| wwise_path::loose_name(entry.path()).map(|name| (entry, name)))
            .ok_or("unexpected physical loose WEM candidate")?;
        let identity = ContainerIdentity::try_new_with_entry(self.archive, entry)?;
        require(
            row.subresource.indices == [u64::try_from(entry.index())?]
                && row.data.pointer("/payload/entry") == Some(&serde_json::to_value(&identity)?)
                && row.data.pointer("/payload/media_id") == Some(&json!(name.id)),
            "original physical loose WEM occurrence",
        )?;
        let (placement, selection) = if identity.raw_path_hex.is_some() {
            (json!({"type":"unavailable"}), "unavailable")
        } else {
            let placement = name.language.map_or_else(
                || json!({"type":"neutral"}),
                |folder| json!({"type":"language","folder":folder}),
            );
            let selection = if entry.index() == self.selected {
                "selected"
            } else {
                "not-selected"
            };
            (placement, selection)
        };
        require(
            row.data.pointer("/payload/placement") == Some(&placement)
                && row.data.pointer("/payload/selection/status") == Some(&json!(selection)),
            "original loose WEM placement and independent export selection",
        )?;
        self.observed += 1;
        Ok(())
    }
}
