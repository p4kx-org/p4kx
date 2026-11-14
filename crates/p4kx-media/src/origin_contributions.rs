// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{FieldOrigin, SourceUse, SourceUses, SourceValue};
use serde::Serialize;

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
    pub(crate) complete: bool,
    pub(crate) cuts: Arc<[OriginCut]>,
}

impl OriginContributions {
    pub(crate) fn unknown(owner: &SourceUses) -> Self {
        Self {
            owner: owner.clone(),
            uses: Arc::from([]),
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

    pub(crate) fn field_origin(&self) -> FieldOrigin {
        self.owner
            .value(SourceValue::Present(()), &self.uses, self.complete)
            .origin()
            .clone()
    }
}
