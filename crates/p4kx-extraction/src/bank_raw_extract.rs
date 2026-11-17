// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use p4kx_audio::EmbeddedWem;

use super::{Bank, BankExtraction, WemFailure, WemFormat, WrittenWem, wem_output_path};
use crate::bank_wem_evidence::BankWemEvidence;
use crate::file_receipt::StagedReceiptPublication;
use crate::origin_file_io::{self, StageDisposition};
use crate::output_file::write_error;
use crate::{
    CancelToken, ConversionError, FileReceipt, OriginArtifactDigest, OriginDigestWriter,
    OriginOutputAddress, write_origin_companion,
};

/// Raw media actually copied from an observed bank and their publication results.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct RawBankExtraction {
    /// The files installed and the members that could not be installed.
    pub extraction: BankExtraction,
    publications: BTreeMap<u32, RawWemPublication>,
}

impl RawBankExtraction {
    /// Returns the receipt publication outcome for an installed media file.
    #[must_use]
    pub fn publication(&self, id: u32) -> Option<&RawWemPublication> {
        self.publications.get(&id)
    }

    /// Reports whether an installed file had a receipt or lock retirement failure.
    #[must_use]
    pub fn has_publication_failures(&self) -> bool {
        self.publications
            .values()
            .any(|publication| !publication.errors.is_empty())
    }
}

/// Receipt publication of one installed raw media file.
#[derive(Debug)]
#[non_exhaustive]
pub struct RawWemPublication {
    /// The receipt installed alongside this media file, when publication succeeded.
    pub receipt: Option<RawWemReceipt>,
    /// Receipt and lock retirement failures after the media file was installed.
    pub errors: Vec<ConversionError>,
}

impl RawWemPublication {
    fn from_staged(
        placed: StagedReceiptPublication,
        receipt_path: &Path,
        receipt_name: PathBuf,
        receipt_digest: Option<OriginArtifactDigest>,
    ) -> io::Result<Self> {
        let (receipt, retirement) = match placed {
            StagedReceiptPublication::Uncommitted(error) => return Err(error),
            StagedReceiptPublication::Committed {
                receipt,
                retirement,
            } => (receipt, retirement),
        };
        let receipt = receipt.and_then(|()| {
            receipt_digest
                .map(|digest| RawWemReceipt {
                    name: receipt_name,
                    digest,
                })
                .ok_or_else(|| io::Error::other("the installed raw media receipt was not measured"))
        });
        let mut errors = Vec::new();
        let receipt = match receipt {
            Ok(receipt) => Some(receipt),
            Err(error) => {
                errors.push(write_error(receipt_path, error));
                None
            }
        };
        if let Err(error) = retirement {
            errors.push(write_error(receipt_path, error));
        }
        Ok(Self { receipt, errors })
    }
}

/// A sibling origin receipt bound to its actual serialized bytes.
#[derive(Debug)]
#[non_exhaustive]
pub struct RawWemReceipt {
    /// The receipt basename within the output folder.
    pub name: PathBuf,
    /// The receipt file's measured byte length and SHA-256.
    pub digest: OriginArtifactDigest,
}

impl Bank {
    /// Copies selected raw media and publishes their source companions.
    ///
    /// # Errors
    /// Returns a folder creation or source protection failure when the destination is unavailable.
    pub fn extract_raw_wems_with_origins(
        &self,
        folder: &Path,
    ) -> Result<RawBankExtraction, ConversionError> {
        let outcomes = self.write_wems(folder, |wem| self.write_observed_wem(wem, folder))?;
        let mut output = RawBankExtraction::default();
        for outcome in outcomes {
            match outcome.result {
                Ok((written, publication)) => {
                    output.publications.insert(written.id, publication);
                    output.extraction.written.push(written);
                }
                Err(error) => output.extraction.failures.push(WemFailure {
                    id: outcome.id,
                    error,
                }),
            }
        }
        Ok(output)
    }

    fn write_observed_wem(
        &self,
        wem: &EmbeddedWem<'_>,
        folder: &Path,
    ) -> Result<(WrittenWem, RawWemPublication), ConversionError> {
        let path = wem_output_path(folder, wem, WemFormat::Wem);
        let protection = self.protection.context()?;
        let file_receipt =
            FileReceipt::for_staged_replacement(&path, protection, CancelToken::new())
                .map_err(|error| write_error(&path, error))?;
        let receipt_path = file_receipt.path();
        let receipt_name = receipt_path.file_name().map(PathBuf::from).ok_or_else(|| {
            write_error(
                receipt_path,
                io::Error::other("raw media receipt has no basename"),
            )
        })?;
        let (inventory, origin) = BankWemEvidence::new(self)
            .and_then(|evidence| evidence.raw_payload(wem))
            .map_err(|error| write_error(receipt_path, io::Error::other(error)))?;
        let address = OriginOutputAddress::OpaquePayload;
        let mut receipt_digest = None;
        let placed = origin_file_io::write_staged(
            &path,
            protection,
            None,
            |writer| {
                let mut measured = OriginDigestWriter::new(writer);
                measured.write_all(wem.bytes())?;
                measured.finish()
            },
            |digest, staged| {
                let result = file_receipt.publish_staged(
                    staged,
                    digest,
                    || Ok(()),
                    |writer, binding| {
                        let mut measured = OriginDigestWriter::new(writer);
                        write_origin_companion(
                            &mut measured,
                            binding,
                            &inventory,
                            [(&address, &origin)],
                        )?;
                        receipt_digest = Some(measured.finish()?);
                        Ok(())
                    },
                );
                Ok(match result {
                    StagedReceiptPublication::Uncommitted(_) => StageDisposition::Cleanup(result),
                    StagedReceiptPublication::Committed { .. } => StageDisposition::Moved(result),
                })
            },
        )
        .map_err(|error| write_error(&path, error))?;
        let publication =
            RawWemPublication::from_staged(placed, receipt_path, receipt_name, receipt_digest)
                .map_err(|error| write_error(&path, error))?;
        let written = WrittenWem {
            id: wem.id(),
            path,
            size: wem.bytes().len() as u64,
            prefetch: wem.is_prefetch(),
        };
        Ok((written, publication))
    }
}
