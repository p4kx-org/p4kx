// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Writes an unencrypted synthetic Archive for packaged-application runtime checks.

use std::error::Error;
use std::path::PathBuf;

use p4kx_testkit::{ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};

fn main() -> Result<(), Box<dyn Error>> {
    let directory = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: native_fixture <absolute-output-directory>")?;
    if !directory.is_absolute() {
        return Err("fixture output directory must be absolute".into());
    }
    std::fs::create_dir_all(&directory)?;
    let text = "P4KX native runtime: Grüezi 日本語 🚀 e\u{301}\nsecond line\twith tab\n";
    let xml = TestElement::new("NativeSmoke")
        .attribute("value", "synthetic")
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/hello.txt", text.as_bytes().to_vec()))
        .entry(TestEntry::stored("Data/empty.txt", Vec::new()))
        .entry(TestEntry::zstd("Data/smoke.xml", xml))
        .write_to(directory.join("native-smoke.p4k"))?;
    std::fs::write(directory.join("hello.txt"), text)?;
    Ok(())
}
