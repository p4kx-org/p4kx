// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::model::{CafImageRecord, CafImageWord};

use super::super::origin as proof;
use super::ImageSource;
use crate::{MediaError, MediaWriter};

pub(super) struct CafPathHashes {
    pub(super) caf: CafImageWord,
    pub(super) dba: CafImageWord,
}

impl ImageSource<'_, '_> {
    pub(super) fn hashes(
        &self,
        record: CafImageRecord<'_>,
        prepared: &mut crate::origin_prepared::PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<CafPathHashes, MediaError> {
        let missing =
            || MediaError::Contract("validated CAF image record has no path hashes".into());
        let caf = record.caf_path_crc32().ok_or_else(missing)?;
        let dba = record.dba_path_crc32().ok_or_else(missing)?;
        for (path, label, word) in [
            ("/data/payload/caf_path_crc32", "cafPathHash", caf),
            ("/data/payload/dba_path_crc32", "dbaPathHash", dba),
        ] {
            let source =
                self.origins
                    .payload(word.range(), label, SourceUseRole::Payload, writer)?;
            proof::at(prepared, path, source, writer)?;
        }
        Ok(CafPathHashes { caf, dba })
    }
}
