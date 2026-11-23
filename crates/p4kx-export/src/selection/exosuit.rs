// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ENTITY_TYPE;

mod controls;
mod proof;

pub(crate) use controls::{admit as admit_exosuit, supported as exosuit_control_supported};
pub(crate) use proof::observe as observe_exosuit_control;

use super::starts_with_folder;
use crate::record_fields::unqualified_class_name;

const ACTOR_FOLDER: &str = "libs/foundry/records/actor/actors/";
const SEAT_FOLDER: &str = "libs/foundry/records/entities/scitem/vehicles/seat/";

#[derive(Clone, Copy)]
pub(crate) enum Exosuit {
    Atls,
    Geo,
    Ikti,
    IktiGeo,
}

impl Exosuit {
    pub(crate) fn by_path(path: &str) -> Option<Self> {
        let class = strip_folder(path, ACTOR_FOLDER)?;
        [Self::Atls, Self::Geo, Self::Ikti, Self::IktiGeo]
            .into_iter()
            .find(|owner| class.eq_ignore_ascii_case(owner.class()))
    }

    pub(crate) fn from_header(path: &str, record_name: &str, source_type: &str) -> Option<Self> {
        let owner = Self::by_path(path)?;
        (source_type == ENTITY_TYPE
            && unqualified_class_name(record_name, source_type).eq_ignore_ascii_case(owner.class()))
        .then_some(owner)
    }

    pub(crate) fn matches_seat(self, path: &str) -> bool {
        let expected = match self {
            Self::Atls => "argo_atls_powersuit_scitem_seat_driver",
            Self::Geo => "argo_atls_geo_scitem_seat_driver",
            Self::Ikti | Self::IktiGeo => "argo_atls_ikti_scitem_seat_driver",
        };
        strip_folder(path, SEAT_FOLDER).is_some_and(|class| class.eq_ignore_ascii_case(expected))
    }

    const fn class(self) -> &'static str {
        match self {
            Self::Atls => "argo_atls",
            Self::Geo => "argo_atls_geo",
            Self::Ikti => "argo_atls_ikti",
            Self::IktiGeo => "argo_atls_geo_ikti",
        }
    }
}

fn strip_folder<'path>(path: &'path str, folder: &str) -> Option<&'path str> {
    starts_with_folder(path, folder).then(|| &path[folder.len()..])
}
