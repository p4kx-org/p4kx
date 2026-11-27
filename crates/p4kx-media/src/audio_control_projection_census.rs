// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::cryxml::SourceDocument;

use crate::audio_control_payload::{self as wire, AtlPayload, AtlPosition};
use crate::audio_control_reference::{self as references, NameDeclaration};
use crate::audio_control_source::AudioControlSource;
use crate::{MediaError, Subresource, origin_error};

use super::{Failure, Sink, walk};

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct AtlCensus {
    pub(crate) documents: u64,
    pub(crate) elements: u64,
    pub(crate) declarations: u64,
    pub(crate) definitions: u64,
    pub(crate) edges: u64,
}

pub(crate) fn census(
    source: &SourceDocument<'_>,
    entry: usize,
    checkpoint: impl FnMut() -> Result<(), MediaError>,
) -> Result<AtlCensus, Failure> {
    let mut sink = CountSink {
        counts: AtlCensus::default(),
        checkpoint,
    };
    walk::project(source, AudioControlSource::Atl, entry, &mut sink)?;
    Ok(sink.counts)
}

struct CountSink<F> {
    counts: AtlCensus,
    checkpoint: F,
}

impl<F: FnMut() -> Result<(), MediaError>> Sink for CountSink<F> {
    fn work(&mut self) -> Result<(), MediaError> {
        (self.checkpoint)()
    }

    fn path(&self) -> &Path {
        Path::new("")
    }

    fn document(&mut self, payload: wire::AtlDocument) -> Result<(), MediaError> {
        self.work()?;
        if payload.repaired_string_bytes {
            return Err(MediaError::Contract(
                "census requires a complete original ATL document".into(),
            ));
        }
        increment(&mut self.counts.documents)
    }

    fn record(
        &mut self,
        _position: AtlPosition,
        _field: &'static str,
        _role: SourceUseRole,
        subresource: &Subresource,
        _payload: &AtlPayload<'_>,
    ) -> Result<(), MediaError> {
        let count = match subresource.namespace.as_str() {
            wire::ELEMENT => &mut self.counts.elements,
            wire::DECLARATION => &mut self.counts.declarations,
            wire::DEFINITION => &mut self.counts.definitions,
            _ => return Ok(()),
        };
        increment(count)
    }

    fn cig_audio_definition(
        &mut self,
        _entry: usize,
        _position: AtlPosition,
        _ancestors: impl Iterator<Item = AtlPosition>,
        _subresource: &Subresource,
        _payload: &AtlPayload<'_>,
    ) -> Result<(), MediaError> {
        Ok(())
    }

    fn name(&mut self, declaration: NameDeclaration<'_>) -> Result<(), MediaError> {
        if references::emits_reference(declaration) {
            increment(&mut self.counts.edges)?;
        }
        Ok(())
    }
}

fn increment(count: &mut u64) -> Result<(), MediaError> {
    *count = count.checked_add(1).ok_or_else(origin_error::counter)?;
    Ok(())
}
