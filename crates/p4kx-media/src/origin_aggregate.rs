// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;

use crate::MediaError;
use crate::origin_capture::OriginCapture;
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_resolver::OriginResolver;
use crate::origin_source;
use crate::origin_subject::OriginSubject;

pub(crate) fn resolve(
    resolver: &OriginResolver<'_>,
    pointer: &str,
    prepared: Option<&PreparedRecordOrigins>,
) -> Result<Option<OriginContributions>, MediaError> {
    match resolver.subject {
        OriginSubject::Record(_) if pointer.is_empty() => record(resolver, prepared).map(Some),
        OriginSubject::Artifact if pointer.is_empty() || pointer == "/digest" => {
            artifact(resolver, pointer).map(Some)
        }
        _ if pointer == "/data"
            || (pointer == "/data/payload" && (shader(resolver) || material(resolver))) =>
        {
            data(resolver, prepared).map(Some)
        }
        _ if pointer.starts_with("/data/")
            && prepared.is_some_and(|prepared| prepared.has_known_descendants(pointer)) =>
        {
            known_data(resolver, pointer, prepared).map(Some)
        }
        _ => Ok(None),
    }
}

fn record(
    resolver: &OriginResolver<'_>,
    prepared: Option<&PreparedRecordOrigins>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = OriginCapture::with_limit(resolver.owner, resolver.detail_bytes);
    capture.include_original(&origin_source::whole(resolver)?)?;
    capture.include_original(&data(resolver, prepared)?)?;
    if resolver.builtin.is_some() {
        capture.include_original(&resolver.resolve("/subresource", None)?)?;
    }
    if let Some(prepared) = prepared {
        prepared.include_known("", &mut capture)?;
    }
    if material(resolver) {
        capture.include_original(&resolver.resolve("/issues", None)?)?;
    } else {
        capture.include_original(&OriginContributions::unknown(resolver.owner))?;
    }
    Ok(capture.finish())
}

fn artifact(
    resolver: &OriginResolver<'_>,
    pointer: &str,
) -> Result<OriginContributions, MediaError> {
    let members: &[&str] = if pointer.is_empty() {
        &["/path", "/size", "/digest"]
    } else {
        &["/digest/algorithm", "/digest/scope", "/digest/hex"]
    };
    let mut capture = OriginCapture::with_limit(resolver.owner, resolver.detail_bytes);
    for member in members {
        capture.include_original(&resolver.resolve(member, None)?)?;
    }
    Ok(capture.finish())
}

fn data(
    resolver: &OriginResolver<'_>,
    prepared: Option<&PreparedRecordOrigins>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = OriginCapture::with_limit(resolver.owner, resolver.detail_bytes);
    if let Some(prepared) = prepared {
        prepared.include_known("/data", &mut capture)?;
    }
    if let Some(builtin) = resolver.builtin {
        if builtin.profile == OriginProfile::ArchiveLayout {
            capture.include_original(&resolver.capture_input(
                &builtin.input,
                builtin.scope,
                "/data",
                SourceUseRole::Payload,
            )?)?;
        } else if builtin.profile != OriginProfile::ArchiveEntry || prepared.is_none() {
            for member in known_members(builtin.profile) {
                capture.include_original(
                    &resolver.resolve(member, if material(resolver) { prepared } else { None })?,
                )?;
            }
        }
    }
    if !material(resolver) {
        capture.include_original(&OriginContributions::unknown(resolver.owner))?;
    }
    Ok(capture.finish())
}

const fn known_members(profile: OriginProfile) -> &'static [&'static str] {
    match profile {
        OriginProfile::ArchiveEntry => &[
            "/data/metadata",
            "/data/compressed_size",
            "/data/uncompressed_size",
            "/data/declared_compression",
            "/data/encrypted",
            "/data/installed_bytes",
            "/data/stored_sha256",
            "/data/stored_sha256_checked_by_reader",
        ],
        OriginProfile::ShaderDefinition => &[
            "/data/payload/file_name",
            "/data/payload/encoding",
            "/data/payload/counts",
        ],
        OriginProfile::ShaderProperty => {
            &["/data/payload/ordinal", "/data/payload/declaration/span"]
        }
        OriginProfile::ShaderDeclaration => &["/data/payload/span"],
        OriginProfile::ShaderFailure
        | OriginProfile::ShaderSourceLimit
        | OriginProfile::ShaderConfigurationFailure => &[
            "/data/payload/file_name",
            "/data/payload/failure",
            "/data/payload/span",
            "/data/payload/counts",
        ],
        OriginProfile::MaterialLibrary => &[
            "/data/payload/path",
            "/data/payload/single",
            "/data/payload/default_palette",
            "/data/payload/unknown_entries",
            "/data/payload/authored",
            "/data/payload/materials",
            "/data/payload/references",
            "/data/payload/shader_references",
            "/data/payload/projection_complete",
        ],
        OriginProfile::MaterialFailure | OriginProfile::MaterialSourceLimit => {
            &["/data/payload/reason"]
        }
        OriginProfile::MaterialConfigurationFailure => &["/data/record_type"],
        OriginProfile::ArchiveLayout | OriginProfile::Unmigrated => &[],
    }
}

fn shader(resolver: &OriginResolver<'_>) -> bool {
    resolver.builtin.is_some_and(|builtin| {
        matches!(
            builtin.profile,
            OriginProfile::ShaderDefinition
                | OriginProfile::ShaderProperty
                | OriginProfile::ShaderDeclaration
                | OriginProfile::ShaderFailure
                | OriginProfile::ShaderSourceLimit
                | OriginProfile::ShaderConfigurationFailure
        )
    })
}
fn material(resolver: &OriginResolver<'_>) -> bool {
    resolver
        .builtin
        .is_some_and(|builtin| crate::origin_material_profile::owns(builtin.profile))
}
fn known_data(
    resolver: &OriginResolver<'_>,
    pointer: &str,
    prepared: Option<&PreparedRecordOrigins>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = OriginCapture::with_limit(resolver.owner, resolver.detail_bytes);
    capture.include_original(
        &resolver.direct(pointer, material(resolver).then_some(prepared).flatten())?,
    )?;
    if let Some(prepared) = prepared {
        prepared.include_known(pointer, &mut capture)?;
    }
    if !material(resolver) {
        capture.include_original(&OriginContributions::unknown(resolver.owner))?;
    }
    Ok(capture.finish())
}
