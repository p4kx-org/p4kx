// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::sound::{WemInfoSource, WemMetadata, WemOriginField, WemOriginPlan};
use p4kx_extraction::{
    Codec, FieldOrigin, SourceCoordinateOmission, SourceEvidenceError, SourceUseRole,
};

use super::super::wem_header_provenance::WemHeaderEvidenceError;
use super::EmbeddedWemEvidence;

/// Original decisions retained from the bounded inspection of one bank row.
#[derive(Debug)]
pub(in crate::preview) enum BankWemInspection {
    Parsed {
        info: WemInfoSource,
        codec: Codec,
        frames: Option<u64>,
    },
    Prefetch,
    Plugin,
    Midi {
        riff: bool,
    },
    Unreadable,
}

impl BankWemInspection {
    pub(in crate::preview) const fn parsed(metadata: &WemMetadata) -> Self {
        Self::Parsed {
            info: *metadata.info_source(),
            codec: metadata.codec(),
            frames: metadata.sample_count(),
        }
    }
}

impl EmbeddedWemEvidence<'_> {
    pub(super) fn row_facts(
        &self,
        inspection: &BankWemInspection,
    ) -> Result<(FieldOrigin, FieldOrigin), WemHeaderEvidenceError> {
        if let BankWemInspection::Parsed {
            info,
            codec,
            frames,
        } = inspection
        {
            let plan = WemOriginPlan::new(info, *codec, *frames);
            let selection = self.riff_selection()?;
            return Ok((
                self.field(&plan, WemOriginField::Codec, &selection)?,
                self.field(&plan, WemOriginField::Duration, &selection)?,
            ));
        }
        let absent = self.disposition(inspection)?;
        Ok((absent.clone(), absent))
    }

    fn disposition(
        &self,
        inspection: &BankWemInspection,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        use BankWemInspection::{Midi, Prefetch, Unreadable};
        use SourceUseRole::Selector;

        let complete = !matches!(inspection, Unreadable);
        let mut inputs = match inspection {
            Prefetch | Midi { riff: true } => self.riff_selection()?,
            _ => Vec::from(self.selectors()?),
        };
        match inspection {
            BankWemInspection::Plugin | Midi { riff: false } => {
                inputs.push(self.riff_word(0, "signature")?);
            }
            Midi { riff: true } => inputs.push(self.riff_word(2, "form")?),
            Unreadable => {
                inputs.push(self.sampled_bytes(Selector)?);
                inputs.push(
                    self.bank
                        .payload
                        .at_opaque(Selector, SourceCoordinateOmission::Unavailable)?,
                );
            }
            _ => {}
        }
        Ok(self
            .bank
            .uses
            .origin(&inputs, complete && self.bank.complete))
    }
}
