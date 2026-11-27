// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_localization::IniEncoding;

use super::LocalizationEntry;
use crate::{
    InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceUse, SourceUseRole,
    SourceUses,
};

pub(crate) struct LocalizationValueEvidence<'entry> {
    entry: &'entry LocalizationEntry,
    offset: u64,
    length: u64,
    encoded: bool,
}

impl LocalizationEntry {
    pub(crate) fn value_evidence(&self, value: &str) -> Option<LocalizationValueEvidence<'_>> {
        let text = self.text();
        let offset = offset_of(text.text(), value)?;
        let (offset, encoded) = match (text.encoding(), text.is_lossy()) {
            (IniEncoding::Utf8, false) => (offset, false),
            (IniEncoding::Utf8Bom, false) => (offset + '\u{feff}'.len_utf8() as u64, false),
            _ => (offset, true),
        };
        Some(LocalizationValueEvidence {
            entry: self,
            offset,
            length: value.len() as u64,
            encoded,
        })
    }

    pub(crate) fn append_value_uses(
        &self,
        value: &str,
        registry: &SourceUses,
        output: &mut Vec<SourceUse>,
    ) -> Result<bool, SourceEvidenceError> {
        self.value_evidence(value)
            .map_or(Ok(false), |value| value.append_uses(registry, output))
    }
}

impl LocalizationValueEvidence<'_> {
    pub(crate) fn append_uses(
        &self,
        registry: &SourceUses,
        output: &mut Vec<SourceUse>,
    ) -> Result<bool, SourceEvidenceError> {
        let original = self.entry.origin();
        let input = original.map_or(SourceInput::Unknown, SourceInput::Archive);
        let scope = registry.observe(input, InputScope::Payload)?;
        let namespace = if self.encoded {
            output.push(scope.at(
                SourceLocation::Subresource {
                    namespace: "globalIniInputBytes".into(),
                    indices: vec![0, self.entry.original_byte_len()],
                    field: "decodeInput".into(),
                },
                SourceUseRole::Conversion,
            )?);
            "globalIniDecodedBytes"
        } else {
            "globalIniBytes"
        };
        output.push(scope.at(
            SourceLocation::Subresource {
                namespace: namespace.into(),
                indices: vec![self.offset, self.length],
                field: "value".into(),
            },
            SourceUseRole::Conversion,
        )?);
        Ok(original.is_some())
    }
}

fn offset_of(text: &str, value: &str) -> Option<u64> {
    let offset = (value.as_ptr() as usize).checked_sub(text.as_ptr() as usize)?;
    let end = offset.checked_add(value.len())?;
    text.get(offset..end).filter(|&inner| inner == value)?;
    Some(offset as u64)
}

#[cfg(test)]
mod tests {
    use super::offset_of;

    #[test]
    fn only_the_borrowed_value_supplies_the_original_byte_coordinate()
    -> Result<(), Box<dyn std::error::Error>> {
        let contents = String::from("abc=größer");
        let stored = contents.strip_prefix("abc=").ok_or("fixture key")?;
        let empty = contents.get(contents.len()..).ok_or("fixture end")?;
        let duplicate = stored.to_owned();
        assert_eq!(offset_of(&contents, stored), Some(4), "stored substring");
        assert_eq!(
            offset_of(&contents, empty),
            Some(contents.len() as u64),
            "empty input retains its physical end position"
        );
        assert_eq!(
            offset_of(&contents, &duplicate),
            None,
            "equal independent bytes have no position in the original"
        );
        Ok(())
    }
}
