// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{BuildManifestOutcome, FieldPath, InputScope, SourceUseRole};

use crate::origin_build::RootBuildOrigin;
use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, RootSource, origin_error};

const FIELDS: &[(&str, &str)] = &[
    ("branch", "Data.Branch"),
    ("version", "Data.Version"),
    ("change_number", "Data.RequestedP4ChangeNum"),
    ("build_id", "Data.BuildId"),
];

pub(crate) fn prepare(
    root: &RootSource,
    prefix: &str,
    profile: OriginProfile,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = writer.record_origins();
    let binding = match root.build_origin.as_ref() {
        Some(binding) if binding.accepts(root, writer)? => Some(binding),
        _ => None,
    };
    if binding.is_some() || profile == OriginProfile::IndexBuild {
        prepared.set_profile(profile);
    }
    let path = FieldPath::parse(prefix).map_err(|cause| origin_error::address(&cause))?;
    if root.build.is_some() {
        fields(root, &path, binding, &mut prepared, writer)?;
    } else {
        let actual = selected_null(binding, writer)?;
        prepared.insert(&path, actual, writer)?;
    }
    Ok(prepared)
}

fn fields(
    root: &RootSource,
    prefix: &FieldPath,
    binding: Option<&RootBuildOrigin>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut aggregate = writer.capture_origins();
    for (field, logical) in FIELDS {
        let actual = authored(binding, logical, writer)?;
        aggregate.include(&actual, writer)?;
        insert(prepared, prefix, field, actual, writer)?;
    }
    let patch = if binding.is_some()
        && root
            .build
            .as_ref()
            .is_some_and(|build| build.public_patch.is_none())
    {
        OriginContributions::independent(&writer.origins.owner)
    } else {
        OriginContributions::unknown(&writer.origins.owner)
    };
    aggregate.include(&patch, writer)?;
    insert(prepared, prefix, "public_patch", patch, writer)?;
    prepared.insert(prefix, aggregate.finish(), writer)
}

fn authored(
    binding: Option<&RootBuildOrigin>,
    logical: &str,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let Some(binding) = binding.filter(|binding| binding.outcome == BuildManifestOutcome::Parsed)
    else {
        return Ok(OriginContributions::unknown(&writer.origins.owner));
    };
    let Some(input) = &binding.input else {
        return Ok(OriginContributions::unknown(&writer.origins.owner));
    };
    let mut capture = writer.capture_origins();
    for role in [SourceUseRole::Payload, SourceUseRole::Conversion] {
        capture.add(
            input,
            InputScope::Payload,
            OriginSite::new("build-manifest", &[], logical),
            role,
            writer,
        )?;
    }
    capture.add(
        input,
        InputScope::Payload,
        OriginSite::new("build-manifest", &[], "required-shape"),
        SourceUseRole::Selector,
        writer,
    )?;
    Ok(capture.finish())
}

fn selected_null(
    binding: Option<&RootBuildOrigin>,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let Some(binding) =
        binding.filter(|binding| binding.outcome == BuildManifestOutcome::Malformed)
    else {
        return Ok(OriginContributions::unknown(&writer.origins.owner));
    };
    let Some(input) = &binding.input else {
        return Ok(OriginContributions::unknown(&writer.origins.owner));
    };
    let mut capture = writer.capture_origins();
    for role in [SourceUseRole::Selector, SourceUseRole::Conversion] {
        capture.add(
            input,
            InputScope::Payload,
            OriginSite::new("build-manifest", &[], "parse-outcome"),
            role,
            writer,
        )?;
    }
    Ok(capture.finish())
}

fn insert(
    prepared: &mut PreparedRecordOrigins,
    prefix: &FieldPath,
    member: &str,
    actual: OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut path = prefix.clone();
    path.push_field(member)
        .map_err(|cause| origin_error::address(&cause))?;
    prepared.insert(&path, actual, writer)
}
