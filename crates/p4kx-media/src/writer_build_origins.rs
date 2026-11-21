// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, BuildManifestObservation, BuildManifestOutcome, InputScope, SourceInput, SourceUseRole,
};

use crate::build_projection;
use crate::origin_build::RootBuildOrigin;
use crate::origin_capture::OriginSite;
use crate::origin_contract::{OriginProfile, OriginStream};
use crate::origin_input::MediaOriginInput;
use crate::origin_resolver::OriginResolver;
use crate::origin_subject::OriginSubject;
use crate::{
    MediaError, MediaIssue, MediaWriter, RootSource, Status, Subresource, origin_build_map,
    origin_error,
};

impl MediaWriter<'_> {
    pub(crate) fn bind_build(
        &self,
        archive: &Archive,
        root: &mut RootSource,
    ) -> Result<(), MediaError> {
        let authority = self.origins.observer()?;
        if !authority.root.is_same_input(archive.input_origin()) {
            return Err(origin_error::foreign());
        }
        let manifest = archive.build_manifest_observation();
        let input = self.build_input(manifest)?;
        root.build_origin = Some(RootBuildOrigin {
            owner: self.origins.owner.clone(),
            root: authority.root,
            input,
            outcome: manifest.outcome(),
            fingerprint: build_projection::fingerprint(
                root.build.as_ref(),
                self.limits.record_bytes,
                self.checkpoint,
            )?,
        });
        Ok(())
    }

    fn build_input(
        &self,
        observed: &BuildManifestObservation,
    ) -> Result<Option<MediaOriginInput>, MediaError> {
        if let Some(origin) = observed.origin() {
            let scope = self
                .origins
                .owner
                .observe(SourceInput::BuildManifest(origin), InputScope::Payload)
                .map_err(origin_error::evidence)?;
            return Ok(Some(MediaOriginInput {
                owner: self.origins.owner.clone(),
                scope,
                details_limited: false,
            }));
        }
        self.build_attempt(observed.outcome())?;
        Ok(None)
    }

    fn build_attempt(&self, outcome: BuildManifestOutcome) -> Result<(), MediaError> {
        let scope = self
            .origins
            .owner
            .observe(SourceInput::Unknown, InputScope::Payload)
            .map_err(origin_error::evidence)?;
        let attempted = MediaOriginInput {
            owner: self.origins.owner.clone(),
            scope,
            details_limited: false,
        };
        let field = match outcome {
            BuildManifestOutcome::Missing => "missing",
            BuildManifestOutcome::Unreadable => "unreadable",
            BuildManifestOutcome::Parsed | BuildManifestOutcome::Malformed | _ => "unavailable",
        };
        self.capture_origins().add(
            &attempted,
            InputScope::Payload,
            OriginSite::new("build-manifest", &[], field),
            SourceUseRole::Attempted,
            self,
        )?;
        Ok(())
    }

    pub(crate) fn archive_root_record(
        &mut self,
        subresource: &Subresource,
        status: Status,
        root: &RootSource,
        issues: &[MediaIssue],
    ) -> Result<(), MediaError> {
        let prepared =
            origin_build_map::prepare(root, "/data/build", OriginProfile::ArchiveBuild, self)?;
        self.record_with_origins(subresource, status, root, issues, prepared)
    }

    pub(crate) fn index_build(&mut self, root: &RootSource) -> Result<(), MediaError> {
        let payload = build_projection::bytes(
            root.build.as_ref(),
            self.limits.record_bytes,
            self.checkpoint,
        )?;
        let prepared =
            origin_build_map::prepare(root, "/source/build", OriginProfile::IndexBuild, self)?;
        let resolver = OriginResolver {
            owner: &self.origins.owner,
            root: None,
            source: None,
            source_prefix: "",
            subject: OriginSubject::IndexBuild,
            artifact: None,
            builtin: None,
            subresource: None,
            detail_bytes: self.limits.record_bytes,
        };
        self.origin_transport.emit(
            OriginStream::IndexBuild,
            OriginProfile::IndexBuild,
            &payload,
            &resolver,
            Some(&prepared),
        )?;
        prepared.require_consumed()
    }
}
