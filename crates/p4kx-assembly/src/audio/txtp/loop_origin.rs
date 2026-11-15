// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::sound::WemOriginSite;
use p4kx_extraction::{Archive, FieldOrigin, InputScope, SourceInput, SourceUses};

use crate::audio::ExportLoopOrigin;

use super::wem_origin;

pub(super) fn field(
    archive: Option<&Archive>,
    sources: &SourceUses,
    evidence: Option<&ExportLoopOrigin>,
    sites: Option<&[WemOriginSite]>,
) -> io::Result<FieldOrigin> {
    let (Some(archive), Some(evidence), Some(sites)) = (archive, evidence, sites) else {
        return Ok(sources.origin(&[], false));
    };
    let Some(entry) = evidence.entry(archive) else {
        return Ok(sources.origin(&[], false));
    };
    let scope = sources
        .observe(SourceInput::ArchiveView(entry), InputScope::Payload)
        .map_err(io::Error::other)?;
    let mut actual = Vec::new();
    if let Some(embedded) = evidence.embedded() {
        wem_origin::embedded_selectors(&scope, embedded, &mut actual)?;
        let prefix = [embedded.section_ordinal(), embedded.entry_ordinal()];
        for site in sites {
            let indices = prefix.iter().chain(site.indices()).copied().collect();
            actual.push(wem_origin::bind(
                &scope,
                site.namespace(),
                indices,
                site.name(),
                site.role(),
            )?);
        }
    } else {
        for site in sites {
            actual.push(site.bind(&scope).map_err(io::Error::other)?);
        }
    }
    let complete = entry.ancestors().all(|ancestor| ancestor.is_some());
    Ok(sources.origin(&actual, complete))
}
