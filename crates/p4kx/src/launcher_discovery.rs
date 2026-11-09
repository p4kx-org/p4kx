// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_channels::{DiscoveryIssue, DiscoveryReport, PublicPatches};

/// Reads public patches after reporting incomplete launcher discovery on stderr.
pub(crate) fn patches() -> io::Result<PublicPatches> {
    reported(PublicPatches::standard_report())
}

/// Returns usable discovery data after delivering its technical warnings.
pub(crate) fn reported<T>(report: DiscoveryReport<T>) -> io::Result<T> {
    reported_to(report, &mut io::stderr().lock())
}

/// Delivers technical discovery warnings without changing the typed source issues.
pub(crate) fn report(issues: &[DiscoveryIssue]) -> io::Result<()> {
    write_issues(&mut io::stderr().lock(), issues)
}

fn reported_to<T>(report: DiscoveryReport<T>, writer: &mut impl Write) -> io::Result<T> {
    write_issues(writer, report.issues())?;
    Ok(report.into_value())
}

fn write_issues(writer: &mut impl Write, issues: &[DiscoveryIssue]) -> io::Result<()> {
    for issue in issues {
        writeln!(writer, "warning: launcher discovery incomplete: {issue}")?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "launcher_discovery_tests.rs"]
mod tests;
