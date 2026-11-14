// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_export::{CatalogSummary, DECOMPRESSED_LIMIT, SizeWarning, UPLOAD_LIMIT};

/// Reports catalog sizes near or above the transfer and decoded-size reference thresholds.
pub(crate) fn warn_about_size(summary: &CatalogSummary) -> io::Result<()> {
    write_size_warnings(
        &mut io::stderr().lock(),
        summary.upload_size(),
        summary.compressed_size().is_some(),
        summary.size(),
    )
}

fn write_size_warnings(
    err: &mut impl Write,
    upload_size: u64,
    compressed: bool,
    decompressed_size: u64,
) -> io::Result<()> {
    if let Some(warning) = SizeWarning::for_size(upload_size) {
        writeln!(
            err,
            "warning: the catalog file is {upload_size} bytes, {} the transfer reference threshold of {UPLOAD_LIMIT} bytes",
            relation(warning)
        )?;
        if !compressed {
            writeln!(
                err,
                "note: write a compressed catalog with --out <file>.json.zst"
            )?;
        }
    }
    if let Some(warning) = SizeWarning::for_decompressed_size(decompressed_size) {
        writeln!(
            err,
            "warning: the catalog is {decompressed_size} bytes decompressed, {} the decoded-size reference threshold of {DECOMPRESSED_LIMIT} bytes",
            relation(warning)
        )?;
    }
    Ok(())
}

const fn relation(warning: SizeWarning) -> &'static str {
    match warning {
        SizeWarning::NearLimit => "close to",
        SizeWarning::OverLimit => "over",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "note: write a compressed catalog with --out <file>.json.zst";

    fn warnings(
        upload_size: u64,
        compressed: bool,
        decompressed_size: u64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut out = Vec::new();
        write_size_warnings(&mut out, upload_size, compressed, decompressed_size)?;
        Ok(String::from_utf8(out)?)
    }

    #[test]
    fn a_plain_file_over_the_upload_limit_advises_json_zst()
    -> Result<(), Box<dyn std::error::Error>> {
        let text = warnings(UPLOAD_LIMIT + 1, false, UPLOAD_LIMIT + 1)?;
        assert!(
            text.contains("over the transfer reference threshold") && text.contains(NOTE),
            "{text}"
        );
        Ok(())
    }

    #[test]
    fn a_plain_file_close_to_the_upload_limit_advises_json_zst()
    -> Result<(), Box<dyn std::error::Error>> {
        let text = warnings(UPLOAD_LIMIT / 10 * 9, false, UPLOAD_LIMIT / 10 * 9)?;
        assert!(
            text.contains("close to the transfer reference threshold") && text.contains(NOTE),
            "{text}"
        );
        Ok(())
    }

    #[test]
    fn a_compressed_file_over_the_limit_gets_no_advice() -> Result<(), Box<dyn std::error::Error>> {
        let text = warnings(UPLOAD_LIMIT + 1, true, 1_000)?;
        assert!(
            text.contains("transfer reference threshold") && !text.contains(NOTE),
            "{text}"
        );
        Ok(())
    }

    #[test]
    fn a_small_catalog_gets_no_warning() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(warnings(1_000, false, 1_000)?, "");
        Ok(())
    }

    #[test]
    fn a_decompressed_size_over_the_limit_is_warned_about() -> Result<(), Box<dyn std::error::Error>>
    {
        let text = warnings(1_000, true, DECOMPRESSED_LIMIT + 1)?;
        assert!(
            text.contains("decompressed") && text.contains("over"),
            "{text}"
        );
        Ok(())
    }
}
