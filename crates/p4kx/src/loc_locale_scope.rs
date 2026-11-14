// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Context, Result};
use p4kx_extraction::{
    IniEncoding, InputScope, LocalizationEntry, SourceInput, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

pub(super) struct LocaleScope<'entry> {
    pub(super) entry: &'entry LocalizationEntry,
    pub(super) scope: SourceScope,
    pub(super) complete: bool,
}

impl<'entry> LocaleScope<'entry> {
    pub(super) fn new(uses: &SourceUses, entry: &'entry LocalizationEntry) -> Result<Self> {
        let origin = entry.origin();
        let input = origin.map_or(SourceInput::Unknown, SourceInput::Archive);
        Ok(Self {
            entry,
            scope: uses.observe(input, InputScope::Payload)?,
            complete: origin
                .is_some_and(|entry| entry.ancestors().all(|ancestor| ancestor.is_some())),
        })
    }

    pub(super) fn location(&self, key: &str, stored: Option<&str>) -> Result<SourceLocation> {
        stored.map_or_else(
            || {
                Ok(SourceLocation::Subresource {
                    namespace: "globalIniLookup".to_owned(),
                    indices: Vec::new(),
                    field: key.trim_prefix('@').to_ascii_lowercase(),
                })
            },
            |value| self.decoded_location(value, "value"),
        )
    }

    pub(super) fn decoded_location(&self, value: &str, field: &str) -> Result<SourceLocation> {
        let range = self
            .entry
            .text()
            .text()
            .as_bytes()
            .subslice_range(value.as_bytes())
            .context("localized datum does not belong to its original Entry")?;
        Ok(SourceLocation::Subresource {
            namespace: "globalIniDecodedText".to_owned(),
            indices: vec![u64::try_from(range.start)?, u64::try_from(range.end)?],
            field: field.to_owned(),
        })
    }

    pub(super) fn text_uses(
        &self,
        location: SourceLocation,
        value: &str,
    ) -> Result<Vec<SourceUse>> {
        let mut inputs = vec![self.scope.at(location.clone(), SourceUseRole::Payload)?];
        if value.contains("\\n") || self.decoded() {
            inputs.push(self.scope.at(location, SourceUseRole::Conversion)?);
        }
        Ok(inputs)
    }

    pub(super) fn decoded(&self) -> bool {
        self.entry.text().is_lossy() || self.entry.text().encoding() != IniEncoding::Utf8
    }
}
