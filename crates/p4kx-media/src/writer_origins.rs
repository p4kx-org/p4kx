// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::InputScope;
use serde::Serialize;

use crate::MediaWriter;
use crate::origin_capture::OriginCapture;
use crate::origin_contract::{OriginProfile, OriginStream};
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_resolver::{BuiltinOrigins, OriginResolver};
use crate::origin_subject::OriginSubject;
use crate::{MediaError, MediaIssue, Source, SourceContext, Status, Subresource, origin_error};

impl MediaWriter<'_> {
    pub(crate) fn capture_origins(&self) -> OriginCapture {
        OriginCapture::with_limit(&self.origins.owner, self.limits.record_bytes)
    }

    pub(crate) fn record_origins(&self) -> PreparedRecordOrigins {
        PreparedRecordOrigins::new(&self.origins.owner)
    }

    pub(crate) fn record_with_origins<T: Serialize>(
        &mut self,
        subresource: &Subresource,
        status: Status,
        data: &T,
        issues: &[MediaIssue],
        origins: PreparedRecordOrigins,
    ) -> Result<(), MediaError> {
        origins.write(self, subresource, status, data, issues)
    }

    pub(crate) fn begin_with_input(
        &mut self,
        active: usize,
        source: Source,
        input: MediaOriginInput,
    ) -> Result<(), MediaError> {
        if !self.origins.owner.is_same_inventory(&input.owner) {
            return Err(origin_error::foreign());
        }
        self.begin(active, source)?;
        self.source_origin = Some(input);
        Ok(())
    }

    pub(crate) fn archive_entry_record<T: Serialize>(
        &mut self,
        context: &SourceContext<'_>,
        subresource: &Subresource,
        data: &T,
    ) -> Result<(), MediaError> {
        let origins = crate::origin_archive::entry(context, subresource, self)?;
        self.record_with_origins(subresource, Status::Complete, data, &[], origins)
    }
    pub(crate) fn archive_layout_record<T: Serialize>(
        &mut self,
        subresource: &Subresource,
        status: Status,
        data: &T,
        issues: &[MediaIssue],
    ) -> Result<(), MediaError> {
        let input = self.origins.directory().cloned().ok_or_else(|| {
            MediaError::Contract("Archive layout has no original directory input".into())
        })?;
        let builtin = BuiltinOrigins {
            profile: OriginProfile::ArchiveLayout,
            input,
            scope: InputScope::Payload,
        };
        self.write_record(subresource, status, data, issues, Some(&builtin), None)
    }

    pub(crate) fn shader_record<T: Serialize>(
        &mut self,
        context: &SourceContext<'_>,
        profile: OriginProfile,
        subresource: &Subresource,
        status: Status,
        data: &T,
        issues: &[MediaIssue],
    ) -> Result<(), MediaError> {
        let scope = match profile {
            OriginProfile::ShaderSourceLimit | OriginProfile::ShaderConfigurationFailure => {
                InputScope::ArchiveDirectoryMetadata
            }
            OriginProfile::ShaderDefinition
            | OriginProfile::ShaderProperty
            | OriginProfile::ShaderDeclaration
            | OriginProfile::ShaderFailure => InputScope::Payload,
            OriginProfile::ArchiveEntry
            | OriginProfile::ArchiveLayout
            | OriginProfile::Unmigrated
            | OriginProfile::MaterialLibrary
            | OriginProfile::MaterialFailure
            | OriginProfile::MaterialSourceLimit
            | OriginProfile::MaterialConfigurationFailure
            | OriginProfile::ArchiveBuild
            | OriginProfile::IndexBuild => {
                return Err(MediaError::Contract(
                    "shader producer used a foreign origin profile".into(),
                ));
            }
        };
        let builtin = BuiltinOrigins {
            profile,
            input: context.origin_input(self)?,
            scope,
        };
        self.write_record(subresource, status, data, issues, Some(&builtin), None)
    }

    pub(crate) fn emit_record_origin(
        &mut self,
        active: usize,
        subresource: &Subresource,
        payload: &[u8],
        builtin: Option<&BuiltinOrigins>,
        prepared: Option<&PreparedRecordOrigins>,
    ) -> Result<(), MediaError> {
        let kind = self
            .kinds
            .get(active)
            .ok_or_else(|| MediaError::Contract("unknown origin provider".into()))?
            .capability
            .kind;
        let resolver = OriginResolver {
            owner: &self.origins.owner,
            root: self.origins.root.as_ref(),
            source: self.source_origin.as_ref(),
            source_prefix: "/source",
            subject: OriginSubject::Record(self.source.as_ref().ok_or_else(|| {
                MediaError::Contract("record origin has no actual Source carrier".into())
            })?),
            artifact: None,
            builtin,
            subresource: Some(subresource),
            detail_bytes: self.limits.record_bytes,
        };
        self.origin_transport.emit(
            OriginStream::Records { kind },
            prepared.map_or_else(
                || builtin.map_or(OriginProfile::Unmigrated, |builtin| builtin.profile),
                PreparedRecordOrigins::profile,
            ),
            payload,
            &resolver,
            prepared,
        )
    }

    pub(crate) fn emit_manifest_origin(
        &mut self,
        stream: OriginStream,
        payload: &[u8],
        input: Option<&MediaOriginInput>,
        artifact_scope: Option<InputScope>,
        subject: OriginSubject<'_>,
    ) -> Result<(), MediaError> {
        if input.is_some_and(|input| !self.origins.owner.is_same_inventory(&input.owner)) {
            return Err(origin_error::foreign());
        }
        let resolver = OriginResolver {
            owner: &self.origins.owner,
            root: self.origins.root.as_ref(),
            source: artifact_scope.is_none().then_some(input).flatten(),
            source_prefix: "",
            subject,
            artifact: input.zip(artifact_scope),
            builtin: None,
            subresource: None,
            detail_bytes: self.limits.record_bytes,
        };
        self.origin_transport
            .emit(stream, OriginProfile::Unmigrated, payload, &resolver, None)
    }
}
