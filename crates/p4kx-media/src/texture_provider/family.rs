// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DdsHeader, DdsRawHeader, DecimalU64, SourceUseRole, TextureMemberCandidate};

use super::family_expected::Expected;
use super::family_inputs::{FamilyInputs, ObservedMembers};
use super::family_origin;
use super::family_payload::{
    Directory, FAMILY, Family, FamilyPayload, MemberFilter, MemberPaths, Relation,
};
use super::header::HeaderRole;
use super::origin::{TextureHeaderOrigins, put};
use crate::origin_capture::OriginCapture;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::source_selection::EntrySelection;
use crate::{
    CandidateInventory, MediaError, MediaIssue, MediaWriter, ReferenceCandidate, Severity,
    SourceContext, Status, Subresource, origin_error,
};

#[path = "family_member.rs"]
mod member;

pub(super) fn project(
    context: &SourceContext<'_>,
    writer: &mut MediaWriter<'_>,
    header: HeaderRole<'_>,
    origin: &TextureHeaderOrigins<'_>,
    plan: Option<(&DdsRawHeader, &DdsHeader, u32)>,
) -> Result<Option<FamilyInputs>, MediaError> {
    let owner = context.origin_input(writer)?;
    let archive = context.directory.index.archive;
    let owner_lossless = archive.raw_path(context.entry.index()).is_none();
    let mut scan = Scan {
        expected: Expected::all(plan.map(|(_, header, count)| (header, count)), writer)?,
        observed: 0,
        inputs: writer.texture_mip.map(|_| ObservedMembers::default()),
        member_paths: writer.capture_origins(),
        lossless: owner_lossless,
        owner_lossless,
        header,
        context,
        owner: &owner,
    };
    let inventory = context.directory.visit_texture_family(
        header.family,
        writer,
        |candidate, reference, writer| scan.visit(&candidate, reference, writer),
    )?;
    let surveyed = family_origin::directory(archive, writer)?;
    let mut capture = writer.capture_origins();
    capture.include(&surveyed, writer)?;
    capture.include(
        &origin.entry_field("path", SourceUseRole::Selector, writer)?,
        writer,
    )?;
    let inventory_origin = capture.finish();
    let complete = scan.lossless && matches!(inventory, CandidateInventory::Complete);
    let paths = std::mem::replace(&mut scan.member_paths, writer.capture_origins()).finish();
    let mut membership = writer.capture_origins();
    membership.include(&inventory_origin, writer)?;
    membership.include(&paths, writer)?;
    let membership = membership.finish();
    scan.summary(inventory, &inventory_origin, &membership, writer)?;
    if let Some((raw, _, _)) = plan {
        let layout = origin.split_layout(raw, writer)?;
        let lookup = if complete {
            &inventory_origin
        } else {
            &membership
        };
        for part in scan.expected {
            part.write(&owner, &layout, lookup, complete, writer)?;
        }
    }
    Ok(scan.inputs.map(|inputs| {
        inputs.finish(
            inventory,
            scan.lossless,
            if complete {
                inventory_origin
            } else {
                membership
            },
        )
    }))
}

struct Scan<'a, 'archive> {
    expected: Vec<Expected>,
    inputs: Option<ObservedMembers>,
    observed: u64,
    member_paths: OriginCapture,
    lossless: bool,
    owner_lossless: bool,
    header: HeaderRole<'a>,
    context: &'a SourceContext<'archive>,
    owner: &'a MediaOriginInput,
}

impl Scan<'_, '_> {
    fn visit(
        &mut self,
        candidate: &TextureMemberCandidate<'_>,
        reference: ReferenceCandidate,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        self.observed = self
            .observed
            .checked_add(1)
            .ok_or_else(origin_error::counter)?;
        let archive = self.context.directory.index.archive;
        let relation = Relation::of(candidate.relation, self.owner_lossless);
        self.lossless &= !matches!(relation, Relation::UnprovedPath);
        if let Some(inputs) = &mut self.inputs {
            inputs.observe(relation, candidate.entry);
        }
        let physical = writer.origins.input(archive, candidate.entry)?;
        let paths = member::relationship(
            self.owner,
            self.context.entry.index(),
            &physical,
            candidate.entry.index(),
            writer,
        )?;
        self.member_paths.include(&paths, writer)?;
        let size = family_origin::entry(
            &physical,
            candidate.entry.index() as u64,
            "uncompressedSize",
            SourceUseRole::Payload,
            writer,
        )?;
        if let Some(expected) = relation
            .number(self.header.alpha)
            .and_then(|number| self.expected.get_mut((number - 1) as usize))
        {
            expected.observe(candidate.entry.uncompressed_size(), &paths, &size, writer)?;
        }
        member::MemberProjection {
            input: &physical,
            entry: candidate.entry,
            relation,
            reference,
            filter: self.member_filter(candidate.entry.path()),
            relationship: paths,
            size,
            key_missing: !archive.has_key(),
        }
        .write(self.owner, writer)
    }

    fn member_filter(&self, path: &str) -> MemberFilter {
        let directory = self.context.directory;
        match directory.selection.classify(directory.chain, path) {
            EntrySelection::Included => MemberFilter::Included,
            EntrySelection::Unmatched => MemberFilter::Unmatched,
            EntrySelection::Excluded => MemberFilter::Excluded,
        }
    }

    fn summary(
        &self,
        inventory: CandidateInventory,
        inventory_evidence: &OriginContributions,
        membership_evidence: &OriginContributions,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut prepared = family_origin::prepare(self.owner)?;
        for field in ["observed_members", "member_paths"] {
            put(
                &mut prepared,
                &format!("/data/payload/{field}"),
                membership_evidence.clone(),
                writer,
            )?;
        }
        put(
            &mut prepared,
            "/data/payload/directory",
            inventory_evidence.clone(),
            writer,
        )?;
        put(
            &mut prepared,
            "/subresource/indices",
            OriginContributions::independent(&self.owner.owner),
            writer,
        )?;
        let (directory, issues) = match inventory {
            CandidateInventory::Complete => (Directory::Complete, None),
            CandidateInventory::Unavailable(reason) => {
                put(&mut prepared, "/issues", inventory_evidence.clone(), writer)?;
                let issue = MediaIssue::new(
                    "texture.family.incomplete-directory",
                    "archive-directory",
                    Severity::Warning,
                    "The owning Archive directory may omit physical DDS family members.",
                );
                (Directory::Incomplete { reason }, Some(issue))
            }
        };
        prepared.write(
            writer,
            &Subresource::new(FAMILY, Vec::new()),
            if issues.is_none() {
                Status::Complete
            } else {
                Status::Partial
            },
            &FamilyPayload::Family(Family {
                observed_members: DecimalU64::new(self.observed),
                directory,
                member_paths: if self.lossless {
                    MemberPaths::Lossless
                } else {
                    MemberPaths::Unproved
                },
            }),
            issues.as_slice(),
        )
    }
}
