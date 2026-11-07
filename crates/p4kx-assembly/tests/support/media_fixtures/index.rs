// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ENTITY_TYPE, Entities, LoadoutFiles};
use p4kx_extraction::{Archive, Guid, Record};
use p4kx_testkit::media_fixture::{self, Candidate, Choice};

use super::{Case, Prepared, SourceCheck, observer, probe, source_checks};

const MAX_ENTITY_CANDIDATES: usize = 1_000_000;
const MAX_IDENTITY_BYTES: usize = 4_096;
const ENTITY_NAME_PREFIX: &str = "EntityClassDefinition.";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Identity {
    pub(crate) guid: Guid,
    pub(crate) record_path: String,
    pub(crate) record_index: u32,
}

pub(crate) struct Index<'f> {
    entities: Arc<Entities<'f>>,
    candidates: Vec<Candidate<Identity>>,
    files: &'f dyn LoadoutFiles,
}

impl<'f> Index<'f> {
    pub(crate) fn new(
        entities: Arc<Entities<'f>>,
        files: &'f dyn LoadoutFiles,
    ) -> super::Result<Self> {
        let mut candidates = Vec::new();
        for record in entities.forge().records() {
            if record.type_name()? != ENTITY_TYPE {
                continue;
            }
            let class_name = record
                .name()?
                .strip_prefix(ENTITY_NAME_PREFIX)
                .ok_or(SourceCheck::SourceIdentity)?;
            let record_path = record.record_path()?;
            source_checks::require(
                !class_name.is_empty()
                    && class_name.len() <= MAX_IDENTITY_BYTES
                    && record_path.len() <= MAX_IDENTITY_BYTES
                    && candidates.len() < MAX_ENTITY_CANDIDATES,
                SourceCheck::SourceScope,
            )?;
            candidates.push(Candidate::new(
                Identity {
                    guid: record.guid(),
                    record_path: record_path.to_owned(),
                    record_index: record.index(),
                },
                class_name.to_owned(),
            ));
        }
        Ok(Self {
            entities,
            candidates,
            files,
        })
    }

    pub(crate) fn choose(
        &self,
        archive: &Archive,
        case: &Case,
    ) -> super::Result<Option<Choice<Identity, Prepared>>> {
        Ok(media_fixture::select(
            case.preferred,
            &self.candidates,
            |candidate| case.accepts_family(&candidate.class_name),
            |candidate| {
                let record = self.record(candidate)?;
                let canonical = observer::canonical(&record)?;
                probe::prepare(
                    archive,
                    self.files,
                    Arc::clone(&self.entities),
                    &record,
                    &canonical,
                    case,
                )
            },
        )?)
    }

    pub(crate) fn record(&self, candidate: &Candidate<Identity>) -> super::Result<Record<'f>> {
        let identity = &candidate.identity;
        let record = self
            .entities
            .forge()
            .record(identity.record_index)
            .ok_or(SourceCheck::SourceIdentity)?;
        source_checks::require(
            record.guid() == identity.guid
                && record.record_path()? == identity.record_path
                && record.type_name()? == ENTITY_TYPE
                && record.name()?.strip_prefix(ENTITY_NAME_PREFIX)
                    == Some(candidate.class_name.as_str()),
            SourceCheck::SourceIdentity,
        )?;
        Ok(record)
    }
}
