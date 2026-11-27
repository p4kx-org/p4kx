// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::{BankHircField, BankMusicFadeFields};

use crate::MediaError;
use crate::audio_bank_music_transition_rule_payload::{TransitionFade, TransitionSignedWord};
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_origins::BankOrigins;

#[derive(Clone, Copy)]
pub(crate) enum FadeSite {
    Source,
    Destination,
    ObjectIn,
    ObjectOut,
}

struct Names {
    path: &'static str,
    time: &'static str,
    curve: &'static str,
    offset: &'static str,
}

impl FadeSite {
    const fn names(self) -> Names {
        match self {
            Self::Source => Names {
                path: "source/fade",
                time: "musicTransitionSourceTimeBits",
                curve: "musicTransitionSourceCurve",
                offset: "musicTransitionSourceOffsetBits",
            },
            Self::Destination => Names {
                path: "destination/fade",
                time: "musicTransitionDestinationTimeBits",
                curve: "musicTransitionDestinationCurve",
                offset: "musicTransitionDestinationOffsetBits",
            },
            Self::ObjectIn => Names {
                path: "transition_object/fade_in",
                time: "musicTransitionObjectFadeInTimeBits",
                curve: "musicTransitionObjectFadeInCurve",
                offset: "musicTransitionObjectFadeInOffsetBits",
            },
            Self::ObjectOut => Names {
                path: "transition_object/fade_out",
                time: "musicTransitionObjectFadeOutTimeBits",
                curve: "musicTransitionObjectFadeOutCurve",
                offset: "musicTransitionObjectFadeOutOffsetBits",
            },
        }
    }
}

pub(crate) fn read(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: &BankMusicFadeFields,
    site: FadeSite,
) -> Result<Option<TransitionFade>, MediaError> {
    let Some(time) = &original.time_bits else {
        return Ok(None);
    };
    let names = site.names();
    let path = names.path;
    Ok(Some(TransitionFade {
        time: signed(sites, prepared, time, &format!("{path}/time"), names.time)?,
        curve: sites.scalar(
            prepared,
            &format!("{path}/curve"),
            original.curve.as_ref(),
            names.curve,
        )?,
        offset: original
            .offset_bits
            .as_ref()
            .map(|word| {
                signed(
                    sites,
                    prepared,
                    word,
                    &format!("{path}/offset"),
                    names.offset,
                )
            })
            .transpose()?,
    }))
}

fn signed(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: &BankHircField<u32>,
    path: &str,
    label: &'static str,
) -> Result<TransitionSignedWord, MediaError> {
    sites.field(prepared, &format!("{path}/bits"), &original.range, label)?;
    sites.converted(
        prepared,
        &format!("{path}/signed_value"),
        &original.range,
        "musicTransitionSignedWord",
    )?;
    Ok(TransitionSignedWord {
        bits: original.into(),
        signed_value: i32::from_le_bytes(original.value.to_le_bytes()),
    })
}
