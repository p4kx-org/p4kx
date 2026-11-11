// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_update::ReleaseCheck;

pub(crate) fn write(json: bool) -> anyhow::Result<()> {
    let release = ReleaseCheck::github().newer_than(env!("CARGO_PKG_VERSION"))?;
    let mut output = io::stdout().lock();
    if json {
        let update = release.as_ref().map(
            |release| serde_json::json!({ "version": release.version, "url": release.link() }),
        );
        writeln!(output, "{}", serde_json::json!({ "update": update }))?;
    } else if let Some(release) = release {
        writeln!(
            output,
            "P4KX {} is available at {}",
            release.version,
            release.link()
        )?;
    } else {
        writeln!(output, "P4KX is up to date.")?;
    }
    Ok(())
}
