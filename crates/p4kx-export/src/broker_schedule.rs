// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The dates a mission broker entry is offered on.

use p4kx_extraction::StructValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::broker_values::{Month, RepeatType};
use crate::contract::every_field_required;
use crate::mission_layers::one;
use crate::record_fields::{integer, structs, text};
use crate::template_issues::Unknowns;

/// A date and time of day as stored; a part the Record does not state is `null`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ScheduleMoment {
    /// The year.
    pub(crate) year: Option<i64>,
    /// The month.
    pub(crate) month: Option<Month>,
    /// The day of the month.
    pub(crate) day: Option<i64>,
    /// The hour.
    pub(crate) hour: Option<i64>,
    /// The minute.
    pub(crate) minute: Option<i64>,
    /// The second.
    pub(crate) second: Option<i64>,
}

/// A window an entry is offered in (`availableDateSchedule`).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ScheduleWindow {
    /// The `start` of the window.
    pub(crate) start: ScheduleMoment,
    /// The `end` of the window.
    pub(crate) end: ScheduleMoment,
    /// The `repeatEnd`: the date after which the window no longer repeats; no time of day.
    pub(crate) repeat_end: ScheduleMoment,
    /// The `repeatType`.
    pub(crate) repeat_type: Option<RepeatType>,
}

/// Reads the `availableDateSchedule` of `root`.
pub(crate) fn schedule(unknowns: &Unknowns, root: Option<&StructValue<'_>>) -> Vec<ScheduleWindow> {
    structs(root, "availableDateSchedule")
        .map(|window| {
            let window = Some(window);
            ScheduleWindow {
                start: moment(unknowns, one(window, "start")),
                end: moment(unknowns, one(window, "end")),
                repeat_end: date(unknowns, one(window, "repeatEnd")),
                repeat_type: unknowns.parse(
                    "repeat type",
                    text(window, "repeatType"),
                    RepeatType::parse,
                ),
            }
        })
        .collect()
}

/// Reads a `date` and a `time` struct.
fn moment(unknowns: &Unknowns, parent: Option<&StructValue<'_>>) -> ScheduleMoment {
    let time = one(parent, "time");
    ScheduleMoment {
        hour: integer(time, "hour"),
        minute: integer(time, "minute"),
        second: integer(time, "second"),
        ..date(unknowns, one(parent, "date"))
    }
}

/// Reads a struct of `year`, `month` and `day`.
fn date(unknowns: &Unknowns, parent: Option<&StructValue<'_>>) -> ScheduleMoment {
    ScheduleMoment {
        year: integer(parent, "year"),
        month: unknowns.parse("month", text(parent, "month"), Month::parse),
        day: integer(parent, "day"),
        hour: None,
        minute: None,
        second: None,
    }
}
