// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const UNKNOWN_CHANNEL: &str = "UnknownChannel";
const UNKNOWN_BUILD: &str = "UnknownBuild";
const MAX_ATTEMPTS: u32 = 10_000;
const SECONDS_PER_DAY: u64 = 86_400;

/// Returns the default target folder name `SC-EXPORT-<Channel>-<Build>-<timestamp>`.
///
/// The timestamp is the UTC time `at` in the form `20261018T153012Z`. Unknown parts are named
/// `UnknownChannel` and `UnknownBuild`.
#[must_use]
pub fn default_target_name(channel: Option<&str>, build: Option<&str>, at: SystemTime) -> String {
    format!(
        "SC-EXPORT-{}-{}-{}",
        sanitise(channel.unwrap_or(UNKNOWN_CHANNEL)),
        sanitise(build.unwrap_or(UNKNOWN_BUILD)),
        utc_timestamp(at)
    )
}

/// Creates a new default target folder below the existing folder `parent` and returns its path.
///
/// When an earlier run already created a folder of that name, `-2`, `-3`, … is appended, so a
/// run never writes into the folder of another run.
///
/// # Errors
///
/// Returns the I/O error of creating the folder.
pub fn create_default_target(
    parent: &Path,
    channel: Option<&str>,
    build: Option<&str>,
    at: SystemTime,
) -> io::Result<PathBuf> {
    create_checked_target(parent, channel, build, at, &|candidate| {
        crate::ensure_writable(candidate)
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))
    })
}

/// Creates a default target while checking every candidate against its Archive job.
///
/// # Errors
/// Returns a protection refusal or the I/O error of creating the folder.
pub fn create_default_target_protected(
    parent: &Path,
    channel: Option<&str>,
    build: Option<&str>,
    at: SystemTime,
    protection: &crate::ArchiveWriteProtection,
    cancel: &crate::CancelToken,
) -> io::Result<PathBuf> {
    create_checked_target(parent, channel, build, at, &|candidate| {
        protection
            .ensure_writable(candidate, cancel.flag())
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))
    })
}

fn create_checked_target(
    parent: &Path,
    channel: Option<&str>,
    build: Option<&str>,
    at: SystemTime,
    check: &dyn Fn(&Path) -> io::Result<()>,
) -> io::Result<PathBuf> {
    let name = default_target_name(channel, build, at);
    for attempt in 1..=MAX_ATTEMPTS {
        let candidate = if attempt == 1 {
            parent.join(&name)
        } else {
            parent.join(format!("{name}-{attempt}"))
        };
        check(&candidate)?;
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("every target folder name {name}-N is taken"),
    ))
}

fn sanitise(part: &str) -> String {
    part.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn utc_timestamp(at: SystemTime) -> String {
    let (year, month, day, time) = utc_parts(at);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        time / 3600,
        time / 60 % 60,
        time % 60
    )
}

/// Returns the UTC time `at` in RFC 3339 form to the second, e.g. `2026-10-18T15:30:12Z`; a time
/// before 1970 is given as `1970-01-01T00:00:00Z`.
#[must_use]
pub fn utc_rfc3339(at: SystemTime) -> String {
    let (year, month, day, time) = utc_parts(at);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time / 3600,
        time / 60 % 60,
        time % 60
    )
}

fn utc_parts(at: SystemTime) -> (u64, u64, u64, u64) {
    let seconds = at
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let (year, month, day) = civil_from_days(seconds / SECONDS_PER_DAY);
    (year, month, day, seconds % SECONDS_PER_DAY)
}

/// Converts days since 1970-01-01 to a proleptic Gregorian `(year, month, day)`.
const fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}
