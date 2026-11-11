// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::sound::{WemFormatField, WemMetadata, WemOriginField, WemOriginPlan};

use crate::audio_record_origins as origins;
use crate::audio_wem_projection::WemProjection;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(crate) fn header(
    projection: &WemProjection<'_>,
    metadata: &WemMetadata,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = origins::prepare(projection.input)?;
    let plan = WemOriginPlan::new(
        metadata.info_source(),
        metadata.codec(),
        metadata.sample_count(),
    );
    for (path, field) in [
        ("/data/payload/byte_order", WemOriginField::RiffByteOrder),
        ("/data/payload/format_tag", WemOriginField::FormatTag),
        ("/data/payload/codec", WemOriginField::Codec),
        ("/data/payload/channels", WemOriginField::Channels),
        ("/data/payload/sample_rate", WemOriginField::SampleRate),
        ("/data/payload/sample_count", WemOriginField::SampleCount),
        ("/data/payload/loop_points", WemOriginField::Loop),
    ] {
        origins::field(
            &mut prepared,
            path,
            planned(projection, &plan, field, writer)?,
            writer,
        )?;
    }
    format_fields(
        projection,
        &plan,
        metadata.channel_layout(),
        &mut prepared,
        writer,
    )?;
    origins::subtree(
        &mut prepared,
        "/data/payload/duration",
        planned(projection, &plan, WemOriginField::Duration, writer)?,
        writer,
    )?;
    if metadata.loop_points().is_some() {
        selected_loop(projection, &plan, &mut prepared, writer)?;
    }
    riff_extent(projection, &mut prepared, writer)?;
    projection.indices(
        &mut prepared,
        OriginContributions::independent(&projection.input.owner),
        writer,
    )?;
    Ok(prepared)
}

fn format_fields(
    projection: &WemProjection<'_>,
    plan: &WemOriginPlan<'_>,
    layout: Option<u32>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let present = layout.is_some();
    for (path, field) in [
        (
            "/data/payload/block_align",
            WemOriginField::Format(WemFormatField::BlockAlign),
        ),
        (
            "/data/payload/bits_per_sample",
            WemOriginField::Format(WemFormatField::BitsPerSample),
        ),
        (
            "/data/payload/channel_layout",
            WemOriginField::Format(WemFormatField::ChannelLayout { present }),
        ),
        (
            "/data/payload/channel_mask",
            WemOriginField::Format(WemFormatField::ChannelMask {
                layout_present: present,
            }),
        ),
        (
            "/data/payload/declared_sample_rate",
            WemOriginField::Format(WemFormatField::DeclaredSampleRate),
        ),
    ] {
        origins::field(
            prepared,
            path,
            planned(projection, plan, field, writer)?,
            writer,
        )?;
    }
    Ok(())
}

fn selected_loop(
    projection: &WemProjection<'_>,
    plan: &WemOriginPlan<'_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    for (path, field) in [
        ("/data/payload/loop_points/start", WemOriginField::LoopStart),
        ("/data/payload/loop_points/end", WemOriginField::LoopEnd),
        (
            "/data/payload/loop_points/frames",
            WemOriginField::LoopFrames,
        ),
    ] {
        origins::field(
            prepared,
            path,
            planned(projection, plan, field, writer)?,
            writer,
        )?;
    }
    Ok(())
}

pub(crate) fn planned(
    projection: &WemProjection<'_>,
    plan: &WemOriginPlan<'_>,
    field: WemOriginField,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let sites = plan
        .field(field)
        .map_err(|cause| MediaError::Contract(cause.to_string()))?;
    projection.capture(
        sites.iter().map(|site| {
            (
                OriginSite::new(site.namespace(), site.indices(), site.name()),
                site.role(),
            )
        }),
        writer,
    )
}

fn riff_extent(
    projection: &WemProjection<'_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let order = (
        OriginSite::new("wemRiffHeader", &[], "signature"),
        SourceUseRole::Conversion,
    );
    let declared = (
        OriginSite::new("wemRiffHeader", &[], "riffSize"),
        SourceUseRole::Conversion,
    );
    origins::field(
        prepared,
        "/data/payload/riff_end_exclusive",
        projection.capture([order, declared], writer)?,
        writer,
    )?;
    origins::field(
        prepared,
        "/data/payload/trailing_length",
        projection.capture(
            [order, declared]
                .into_iter()
                .chain((!projection.is_embedded()).then_some((
                    OriginSite::new("media-physical-input", &[], "complete-payload-length"),
                    SourceUseRole::Conversion,
                ))),
            writer,
        )?,
        writer,
    )
}
