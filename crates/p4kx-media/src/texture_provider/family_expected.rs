// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DdsHeader, DecimalU64};

use super::family_origin;
use super::family_payload::{EXPECTED, ExpectedPart, FamilyPayload, PartDisposition};
use super::origin::put;
use crate::origin_capture::OriginCapture;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{MediaError, MediaWriter, Status, Subresource, origin_error};

pub(super) struct Expected {
    number: u32,
    level: u32,
    bytes: u64,
    observed: u64,
    matching_size: bool,
    paths: OriginCapture,
    sizes: OriginCapture,
}

impl Expected {
    pub(super) fn all(
        plan: Option<(&DdsHeader, u32)>,
        writer: &MediaWriter<'_>,
    ) -> Result<Vec<Self>, MediaError> {
        let Some((header, count)) = plan else {
            return Ok(Vec::new());
        };
        (1..=count)
            .map(|number| {
                let level = count - number;
                let bytes = header.mip_size(level).ok_or_else(|| {
                    MediaError::Contract("expected split mip has no decoded extent".into())
                })?;
                Ok(Self {
                    number,
                    level,
                    bytes,
                    observed: 0,
                    matching_size: false,
                    paths: writer.capture_origins(),
                    sizes: writer.capture_origins(),
                })
            })
            .collect()
    }

    pub(super) fn observe(
        &mut self,
        bytes: u64,
        member_path: &OriginContributions,
        member_size: &OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        self.observed = self
            .observed
            .checked_add(1)
            .ok_or_else(origin_error::counter)?;
        self.matching_size = bytes == self.bytes;
        self.paths.include(member_path, writer)?;
        self.sizes.include(member_size, writer)
    }

    pub(super) fn write(
        self,
        owner: &MediaOriginInput,
        layout: &OriginContributions,
        inventory: &OriginContributions,
        complete: bool,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let disposition = match (self.observed, complete, self.matching_size) {
            (2.., _, _) => PartDisposition::Ambiguous,
            (_, false, _) => PartDisposition::Unproved,
            (0, true, _) => PartDisposition::Missing,
            (1, true, true) => PartDisposition::DeclaredSizeMatches,
            (1, true, false) => PartDisposition::DeclaredSizeMismatch,
        };
        let mut prepared = family_origin::prepare(owner)?;
        let mut evidence = self.paths;
        evidence.include(inventory, writer)?;
        evidence.include(layout, writer)?;
        let membership = evidence.finish();
        let compared = compare_evidence(
            &membership,
            self.sizes,
            self.observed == 1 && complete,
            writer,
        )?;
        for field in ["number", "level", "expected_bytes"] {
            put(
                &mut prepared,
                &format!("/data/payload/{field}"),
                layout.clone(),
                writer,
            )?;
        }
        put(
            &mut prepared,
            "/data/payload/matching_members",
            membership,
            writer,
        )?;
        put(&mut prepared, "/data/payload/disposition", compared, writer)?;
        put(
            &mut prepared,
            "/subresource/indices",
            layout.clone(),
            writer,
        )?;
        prepared.write(
            writer,
            &Subresource::new(EXPECTED, vec![u64::from(self.number)]),
            Status::Complete,
            &FamilyPayload::ExpectedPart(ExpectedPart {
                number: self.number,
                level: self.level,
                expected_bytes: DecimalU64::new(self.bytes),
                matching_members: DecimalU64::new(self.observed),
                disposition,
            }),
            &[],
        )
    }
}

fn compare_evidence(
    membership: &OriginContributions,
    mut sizes: OriginCapture,
    used_size: bool,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    if !used_size {
        return Ok(membership.clone());
    }
    sizes.include(membership, writer)?;
    Ok(sizes.finish())
}
