// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{InputScope, SourceUseRole};

use crate::origin_contract::OriginProfile;
use crate::origin_resolver;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ScopeMapping {
    Unmapped,
    BuiltinScope,
    SpecificScope(InputScope),
}

/// Selects only datum families established by the concrete owning producer.
pub(crate) fn scope(profile: OriginProfile, pointer: &str) -> ScopeMapping {
    let Some(data) = pointer.strip_prefix("/data") else {
        return ScopeMapping::Unmapped;
    };
    if !data.is_empty() && !data.starts_with('/') {
        return ScopeMapping::Unmapped;
    }
    match profile {
        OriginProfile::ArchiveEntry => archive_entry(data),
        OriginProfile::ArchiveLayout => ScopeMapping::BuiltinScope,
        OriginProfile::ShaderDefinition => ScopeMapping::SpecificScope(shader_definition(data)),
        OriginProfile::ShaderProperty | OriginProfile::ShaderDeclaration => {
            ScopeMapping::SpecificScope(InputScope::Payload)
        }
        OriginProfile::ShaderFailure
        | OriginProfile::ShaderSourceLimit
        | OriginProfile::ShaderConfigurationFailure => failure_scope(profile, data),
        OriginProfile::Unmigrated => ScopeMapping::Unmapped,
    }
}

fn archive_entry(pointer: &str) -> ScopeMapping {
    if pointer.is_empty()
        || [
            "/metadata",
            "/compressed_size",
            "/uncompressed_size",
            "/declared_compression",
            "/encrypted",
            "/installed_bytes",
            "/stored_sha256",
            "/stored_sha256_checked_by_reader",
        ]
        .iter()
        .any(|prefix| origin_resolver::within(pointer, prefix))
    {
        ScopeMapping::SpecificScope(InputScope::ArchiveDirectoryMetadata)
    } else {
        ScopeMapping::Unmapped
    }
}

fn shader_definition(pointer: &str) -> InputScope {
    if [
        "/payload/file_name",
        "/payload/shader_name",
        "/payload/name_origin",
    ]
    .iter()
    .any(|prefix| origin_resolver::within(pointer, prefix))
    {
        return InputScope::ArchiveDirectoryMetadata;
    }
    InputScope::Payload
}

fn failure_scope(profile: OriginProfile, pointer: &str) -> ScopeMapping {
    if origin_resolver::within(pointer, "/payload/file_name") {
        return ScopeMapping::SpecificScope(InputScope::ArchiveDirectoryMetadata);
    }
    match profile {
        OriginProfile::ShaderFailure
            if ["/payload/failure", "/payload/span"]
                .iter()
                .any(|prefix| origin_resolver::within(pointer, prefix)) =>
        {
            ScopeMapping::SpecificScope(InputScope::Payload)
        }
        OriginProfile::ShaderSourceLimit if origin_resolver::within(pointer, "/payload/span") => {
            ScopeMapping::SpecificScope(InputScope::ArchiveDirectoryMetadata)
        }
        _ => ScopeMapping::Unmapped,
    }
}

pub(crate) fn independent(profile: OriginProfile, pointer: &str) -> bool {
    match profile {
        OriginProfile::ArchiveLayout => origin_resolver::within(pointer, "/data/version"),
        OriginProfile::ShaderDefinition
        | OriginProfile::ShaderProperty
        | OriginProfile::ShaderDeclaration
        | OriginProfile::ShaderFailure
        | OriginProfile::ShaderSourceLimit
        | OriginProfile::ShaderConfigurationFailure => {
            pointer == "/data/record_type"
                || pointer.ends_with("/span/basis")
                || pointer == "/data/payload/name_origin"
                || failure_control(profile, pointer)
        }
        OriginProfile::ArchiveEntry | OriginProfile::Unmigrated => false,
    }
}

fn failure_control(profile: OriginProfile, pointer: &str) -> bool {
    match profile {
        OriginProfile::ShaderFailure => matches!(
            pointer,
            "/data/payload/counts" | "/data/payload/failure/budget" | "/data/payload/failure/limit"
        ),
        OriginProfile::ShaderSourceLimit => {
            matches!(pointer, "/data/payload/counts" | "/data/payload/span/start")
                || origin_resolver::within(pointer, "/data/payload/failure")
        }
        OriginProfile::ShaderConfigurationFailure => {
            pointer == "/data/payload/counts"
                || origin_resolver::within(pointer, "/data/payload/failure")
                || origin_resolver::within(pointer, "/data/payload/span")
        }
        _ => false,
    }
}

pub(crate) fn role(profile: OriginProfile, pointer: &str) -> SourceUseRole {
    let converted = match profile {
        OriginProfile::ArchiveLayout => pointer.contains("/artifact/digest"),
        OriginProfile::ShaderFailure
        | OriginProfile::ShaderSourceLimit
        | OriginProfile::ShaderConfigurationFailure => true,
        OriginProfile::ShaderDefinition => [
            "/data/payload/file_name",
            "/data/payload/shader_name",
            "/data/payload/counts",
        ]
        .iter()
        .any(|prefix| origin_resolver::within(pointer, prefix)),
        OriginProfile::ShaderProperty | OriginProfile::ShaderDeclaration => {
            pointer.contains("/meaning/interpretation")
        }
        _ => false,
    };
    if converted {
        SourceUseRole::Conversion
    } else {
        SourceUseRole::Payload
    }
}
