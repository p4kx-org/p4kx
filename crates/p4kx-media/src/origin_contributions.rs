// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{FieldOrigin, SharedFieldOrigin, SourceUse, SourceUses};
use serde::Serialize;

use crate::{MediaError, origin_error};

/// Local optional-capture loss independent of payload and projection health.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OriginCut {
    InputMetadata,
    UseMetadata,
    FieldUses,
}

/// Immutable actual contributions shared without repeated reference vectors.
#[derive(Debug, Clone)]
pub(crate) struct OriginContributions {
    pub(crate) owner: SourceUses,
    pub(crate) uses: Arc<[SourceUse]>,
    pub(crate) shared: Option<Arc<SharedUseSet>>,
    pub(crate) complete: bool,
    pub(crate) cuts: Arc<[OriginCut]>,
}

impl OriginContributions {
    pub(crate) fn unknown(owner: &SourceUses) -> Self {
        Self {
            owner: owner.clone(),
            uses: Arc::from([]),
            shared: None,
            complete: false,
            cuts: Arc::from([]),
        }
    }

    pub(crate) fn independent(owner: &SourceUses) -> Self {
        Self {
            complete: true,
            ..Self::unknown(owner)
        }
    }

    pub(crate) fn share_uses(mut self) -> Self {
        if self.uses.len() > 1 && self.shared.is_none() {
            let uses = std::mem::replace(&mut self.uses, Arc::from([]));
            let origin = self.owner.shared_origin(&uses, self.complete);
            self.shared = Some(Arc::new(SharedUseSet { uses, origin }));
        }
        self
    }

    pub(crate) fn field_origin(&self) -> Result<FieldOrigin, MediaError> {
        if let Some(shared) = &self.shared {
            return self
                .owner
                .origin_with_shared(&shared.origin, &self.uses, self.complete)
                .map_err(origin_error::evidence);
        }
        Ok(self.owner.origin(&self.uses, self.complete))
    }
}

/// Original uses shared by fields in one physical source scope.
#[derive(Debug)]
pub(crate) struct SharedUseSet {
    pub(crate) uses: Arc<[SourceUse]>,
    origin: SharedFieldOrigin,
}

impl SharedUseSet {
    pub(crate) fn contains(&self, used: &SourceUse) -> bool {
        self.uses
            .binary_search_by(|member| member.reference().cmp(used.reference()))
            .is_ok()
    }
}
