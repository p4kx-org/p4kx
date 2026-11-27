// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Context, Result};
use p4kx_export::RecordForm;
use p4kx_extraction::{DataForge, ObservedTagTree, TagResolver, TagTree};

use crate::forge_input::ForgeInput;

pub(super) enum RecordExportTags {
    Observed(ObservedTagTree),
    Plain(TagTree),
}

impl RecordExportTags {
    pub(super) fn read(
        input: &ForgeInput,
        forge: &DataForge<'_>,
        form: RecordForm,
    ) -> Result<Self> {
        if matches!(
            form,
            RecordForm::Json | RecordForm::Xml | RecordForm::Ndjson
        ) {
            let observed = ObservedTagTree::build(input.shared_entry(), forge, &|| false)?
                .context("the tag tree did not finish")?;
            Ok(Self::Observed(observed))
        } else {
            Ok(Self::Plain(TagTree::build(forge)))
        }
    }

    pub(super) fn resolver(&self) -> &dyn TagResolver {
        match self {
            Self::Observed(tags) => tags,
            Self::Plain(tags) => tags,
        }
    }

    pub(super) const fn observed(&self) -> Option<&ObservedTagTree> {
        match self {
            Self::Observed(tags) => Some(tags),
            Self::Plain(_) => None,
        }
    }
}
