// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Context as _, Result};
use p4kx_extraction::{SourceLocation, SourceUse, SourceUseRole};
use p4kx_screens::AttachmentInput;

use super::Projection;

pub(super) struct AttachmentEvidence {
    sources: Vec<SourceUse>,
    complete: bool,
}

static ROOT: AttachmentEvidence = AttachmentEvidence {
    sources: Vec::new(),
    complete: true,
};

impl AttachmentEvidence {
    pub(super) const fn root() -> &'static Self {
        &ROOT
    }

    pub(super) fn at(routes: &[Self], index: Option<usize>) -> Result<&Self> {
        index.map_or_else(
            || Ok(Self::root()),
            |index| routes.get(index).context("screen attachment origin index"),
        )
    }
}

impl Projection {
    pub(super) fn attachment(&self, route: &AttachmentInput) -> Result<AttachmentEvidence> {
        let mut sources = Vec::new();
        for port in route.ports() {
            for (field, role) in [
                ("Name", SourceUseRole::Conversion),
                ("defaultItem.entityClass", SourceUseRole::Selector),
            ] {
                sources.push(self.scope.at(
                    SourceLocation::DataForge {
                        record_index: port.record_index(),
                        field: format!(
                            "Components[{}].Ports[{}].{field}",
                            port.component_ordinal(),
                            port.port_ordinal(),
                        ),
                    },
                    role,
                )?);
            }
        }
        Ok(AttachmentEvidence {
            sources,
            complete: route.is_complete(),
        })
    }

    pub(super) fn put_attached(
        &mut self,
        path: &str,
        sources: &[SourceUse],
        complete: bool,
        attachment: &AttachmentEvidence,
    ) -> Result<()> {
        let complete = complete && attachment.complete;
        if attachment.sources.is_empty() {
            self.put(path, sources, complete)
        } else {
            let mut combined = attachment.sources.clone();
            combined.extend_from_slice(sources);
            self.put(path, &combined, complete)
        }
    }
}
