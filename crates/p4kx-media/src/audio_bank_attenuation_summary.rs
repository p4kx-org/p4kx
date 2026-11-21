// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::{BankAttenuationCone, BankHircAttenuation};

use super::payload::{AttenuationCone, AttenuationObservations};
use crate::MediaError;
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_origins::BankOrigins;

pub(super) fn observed<R>(
    attenuation: &BankHircAttenuation<'_, R>,
    sites: &NodeSites,
    origins: &mut BankOrigins<'_, '_, '_>,
) -> Result<AttenuationObservations, MediaError> {
    let fields = attenuation.fields();
    let checked = attenuation.checked_range();
    sites.range(origins, "observed/checked_range", &checked)?;
    let unprojected = attenuation.unprojected_range();
    if let Some(range) = &unprojected {
        sites.range(origins, "observed/unprojected_range", range)?;
    }
    if let Some(flags) = &fields.cone_flags {
        sites.converted(origins, "observed/cone_enabled", &flags.range, "coneFlags")?;
    }
    Ok(AttenuationObservations {
        height_spread_flags: sites.scalar(
            origins,
            "observed/height_spread_flags",
            fields.height_spread_flags.as_ref(),
            "heightSpreadFlags",
        )?,
        cone_flags: sites.scalar(
            origins,
            "observed/cone_flags",
            fields.cone_flags.as_ref(),
            "coneFlags",
        )?,
        cone_enabled: fields.cone_enabled(),
        cone: fields
            .cone
            .as_ref()
            .map(|cone| cone_fields(sites, origins, cone))
            .transpose()?,
        curve_count: sites.scalar(
            origins,
            "observed/curve_count",
            fields.curve_count.as_ref(),
            "attenuationCurveCount",
        )?,
        checked_range: (&checked).into(),
        unprojected_range: unprojected.as_ref().map(Into::into),
    })
}

fn cone_fields(
    sites: &NodeSites,
    origins: &mut BankOrigins<'_, '_, '_>,
    cone: &BankAttenuationCone,
) -> Result<AttenuationCone, MediaError> {
    Ok(AttenuationCone {
        inside_degrees: sites.scalar(
            origins,
            "observed/cone/inside_degrees_bits",
            cone.inside_degrees_bits.as_ref(),
            "coneInsideDegreesBits",
        )?,
        outside_degrees: sites.scalar(
            origins,
            "observed/cone/outside_degrees_bits",
            cone.outside_degrees_bits.as_ref(),
            "coneOutsideDegreesBits",
        )?,
        outside_volume: sites.scalar(
            origins,
            "observed/cone/outside_volume_bits",
            cone.outside_volume_bits.as_ref(),
            "coneOutsideVolumeBits",
        )?,
        low_pass: sites.scalar(
            origins,
            "observed/cone/low_pass_bits",
            cone.low_pass_bits.as_ref(),
            "coneLowPassBits",
        )?,
        high_pass: sites.scalar(
            origins,
            "observed/cone/high_pass_bits",
            cone.high_pass_bits.as_ref(),
            "coneHighPassBits",
        )?,
    })
}
