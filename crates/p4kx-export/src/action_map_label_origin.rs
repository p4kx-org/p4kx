// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::{
    IniEncoding, LocalizationEntry, Lookup, SourceLocation, SourceScope, SourceUse, SourceUseRole,
    SourceUses, TextSet,
};

use crate::action_map_source::{location, scope};

pub(crate) struct LabelOrigin<'input> {
    entry: &'input LocalizationEntry,
    texts: &'input TextSet<'input>,
    source: SourceScope,
    complete: bool,
}

impl<'input> LabelOrigin<'input> {
    pub(crate) fn new(
        uses: &SourceUses,
        entry: &'input LocalizationEntry,
        texts: &'input TextSet<'input>,
    ) -> io::Result<Self> {
        let (source, complete) = scope(uses, entry.origin())?;
        Ok(Self {
            entry,
            texts,
            source,
            complete,
        })
    }

    pub(crate) fn contribute(
        &self,
        key: &str,
        inputs: &mut Vec<SourceUse>,
        use_kind: LabelUse,
    ) -> io::Result<bool> {
        let language = self.entry.language();
        let lookup = self.texts.lookup(key, language, None);
        if matches!(lookup, Lookup::Placeholder) {
            return Ok(true);
        }
        let location = if let Some(value) = self.texts.text(key, language) {
            self.value_location(value)?
        } else {
            let selector = inputs
                .first()
                .ok_or_else(|| io::Error::other("action label lookup has no original selector"))?;
            SourceLocation::LocalizationLookup {
                language,
                selector: selector.reference().clone(),
            }
        };
        inputs.push(
            self.source
                .at(location.clone(), SourceUseRole::Selector)
                .map_err(io::Error::other)?,
        );
        if matches!(use_kind, LabelUse::Output)
            && let Some(text) = lookup.text()
        {
            inputs.push(
                self.source
                    .at(location.clone(), SourceUseRole::Payload)
                    .map_err(io::Error::other)?,
            );
            if text.contains("\\n")
                || self.entry.text().is_lossy()
                || self.entry.text().encoding() != IniEncoding::Utf8
            {
                inputs.push(
                    self.source
                        .at(location, SourceUseRole::Conversion)
                        .map_err(io::Error::other)?,
                );
            }
        }
        Ok(self.complete)
    }

    fn value_location(&self, value: &str) -> io::Result<SourceLocation> {
        let range = self
            .entry
            .text()
            .text()
            .as_bytes()
            .subslice_range(value.as_bytes())
            .ok_or_else(|| {
                io::Error::other("action label did not come from the retained language Entry")
            })?;
        Ok(location(
            "globalIniDecodedText",
            vec![range.start as u64, range.end as u64],
            "value",
        ))
    }
}

#[derive(Clone, Copy)]
pub(crate) enum LabelUse {
    Output,
    ResolutionCount,
}
