// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{InputScope, SourceUseRole};

use crate::origin_contract::OriginProfile;
use crate::origin_data::ScopeMapping;
use crate::origin_resolver;

pub(crate) const fn owns(profile: OriginProfile) -> bool {
    matches!(
        profile,
        OriginProfile::MaterialLibrary
            | OriginProfile::MaterialFailure
            | OriginProfile::MaterialSourceLimit
            | OriginProfile::MaterialConfigurationFailure
    )
}

pub(crate) fn scope(profile: OriginProfile, data: &str) -> ScopeMapping {
    match profile {
        OriginProfile::MaterialLibrary => library_scope(data),
        OriginProfile::MaterialFailure if origin_resolver::within(data, "/payload/reason") => {
            ScopeMapping::SpecificScope(InputScope::Payload)
        }
        OriginProfile::MaterialSourceLimit if origin_resolver::within(data, "/payload/reason") => {
            ScopeMapping::SpecificScope(InputScope::ArchiveDirectoryMetadata)
        }
        _ => ScopeMapping::Unmapped,
    }
}

fn library_scope(data: &str) -> ScopeMapping {
    if origin_resolver::within(data, "/payload/path") {
        return ScopeMapping::SpecificScope(InputScope::ArchiveDirectoryMetadata);
    }
    let source = data.is_empty()
        || data == "/payload"
        || [
            "/payload/authored",
            "/payload/default_palette",
            "/payload/single",
            "/payload/unknown_entries",
            "/payload/projection_complete",
        ]
        .iter()
        .any(|prefix| origin_resolver::within(data, prefix))
        || matches!(
            data,
            "/payload/materials" | "/payload/references" | "/payload/shader_references"
        );
    if source {
        return ScopeMapping::SpecificScope(InputScope::Payload);
    }
    let Some(material) = data.strip_prefix("/payload/materials/") else {
        return ScopeMapping::Unmapped;
    };
    let (index, field) = material.split_once('/').unwrap_or((material, ""));
    if index.parse::<usize>().is_err() {
        return ScopeMapping::Unmapped;
    }
    if field.is_empty()
        || matches!(field, "index" | "interpreted_surface")
        || origin_resolver::within(field, "authored")
    {
        ScopeMapping::SpecificScope(InputScope::Payload)
    } else {
        ScopeMapping::Unmapped
    }
}

pub(crate) fn independent(profile: OriginProfile, pointer: &str) -> bool {
    if pointer == "/data/record_type" || origin_resolver::within(pointer, "/subresource") {
        return true;
    }
    match profile {
        OriginProfile::MaterialConfigurationFailure => origin_resolver::within(pointer, "/data"),
        OriginProfile::MaterialFailure | OriginProfile::MaterialSourceLimit => matches!(
            pointer,
            "/data/payload/reason/limit" | "/data/payload/reason/what"
        ),
        OriginProfile::MaterialLibrary => [
            "/interpreted_surface/basis",
            "/interpreted_surface/tinted",
            "/interpreted_surface/decal_colours",
            "/interpreted_surface/diffuse_map_is_grey",
        ]
        .iter()
        .any(|suffix| pointer.ends_with(suffix)),
        _ => false,
    }
}

pub(crate) fn role(profile: OriginProfile, pointer: &str) -> SourceUseRole {
    if profile != OriginProfile::MaterialLibrary {
        return SourceUseRole::Conversion;
    }
    if pointer.ends_with("/projection_complete") || pointer == "/data/payload/path" {
        SourceUseRole::Conversion
    } else if pointer.contains("/authored") || pointer == "/data/payload/default_palette" {
        SourceUseRole::Payload
    } else {
        SourceUseRole::Selector
    }
}

pub(crate) fn issue_independent(profile: OriginProfile, pointer: &str) -> bool {
    profile != OriginProfile::MaterialLibrary || pointer.ends_with("/severity")
}
pub(crate) fn authored(pointer: &str) -> bool {
    origin_resolver::within(pointer, "/data/payload/authored")
        || (pointer.starts_with("/data/payload/materials/") && pointer.contains("/authored"))
}
