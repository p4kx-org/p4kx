// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use flate2::bufread::GzDecoder;
use serde_json::Value;
use sha2::{Digest, Sha256};

type Checked<T = ()> = Result<T, Box<dyn Error>>;

/// Checks the package manifest, material companions and receipt against synthetic library paths.
///
/// Returns the number of files covered by the receipt plus the receipt itself.
///
/// # Errors
/// Returns malformed fixture metadata, missing files or read and decompression errors.
///
/// # Panics
/// Panics when the emitted document contradicts the expected fixture or file digests.
pub fn check_entity_package_sources(
    root: &Path,
    manifest: &Value,
    expected_libraries: &[&str],
) -> Checked<usize> {
    let libraries = array(manifest, "material_libraries")?;
    assert_eq!(
        libraries.len(),
        expected_libraries.len(),
        "one companion per used authored library"
    );
    let mut paths = vec!["scene.json"];
    for collection in ["meshes", "textures"] {
        for item in array(manifest, collection)? {
            paths.push(text(item, "file")?);
        }
    }
    for (ordinal, (library, source)) in libraries.iter().zip(expected_libraries).enumerate() {
        let reference = library
            .get("authored_file")
            .ok_or("original library companion")?;
        let path = text(reference, "file")?;
        assert_eq!(
            path,
            format!("material-sources/s{ordinal:04}.jsonl.gz"),
            "companion identifies its original library row"
        );
        assert_eq!(
            reference.get("schema_version").and_then(Value::as_u64),
            Some(1),
            "versioned material companion"
        );
        assert!(
            library.get("authored").is_none() && library.get("authored_origin").is_none(),
            "authored source and origins are carried in the companion"
        );
        check_library(&root.join(path), ordinal, source)?;
        paths.push(path);
    }
    paths.sort_unstable();
    check_receipt(root, manifest, &paths)?;
    Ok(paths.len() + 1)
}

fn check_library(path: &Path, ordinal: usize, expected_source: &str) -> Checked {
    let mut rows = BufReader::new(GzDecoder::new(BufReader::new(File::open(path)?)))
        .lines()
        .map(|line| Ok::<_, Box<dyn Error>>(serde_json::from_str::<Value>(&line?)?));
    let header = rows.next().ok_or("material companion header")??;
    assert_eq!(text(&header, "type")?, "header", "first companion row");
    assert_eq!(
        text(&header, "format")?,
        "p4kx-blender-material-source",
        "companion format"
    );
    assert_eq!(
        header.get("library").and_then(Value::as_u64),
        Some(ordinal as u64),
        "authored library ordinal"
    );
    assert_eq!(
        header.get("version").and_then(Value::as_u64),
        Some(1),
        "companion payload version"
    );
    let payload = rows.next().ok_or("authored source payload")??;
    assert_eq!(
        text(&payload, "type")?,
        "library",
        "second companion row carries authored values"
    );
    assert_eq!(
        payload
            .pointer("/authored/root/name")
            .and_then(Value::as_str),
        Some("Material"),
        "authored root is preserved"
    );
    assert_eq!(
        payload
            .pointer("/authored/encoding")
            .and_then(Value::as_str),
        Some("cry-xml"),
        "binary XML source encoding"
    );
    check_origins(rows, expected_source)
}

fn check_origins(rows: impl Iterator<Item = Checked<Value>>, expected_source: &str) -> Checked {
    let mut counts = [0_u64; 3];
    let mut source = None;
    let mut uses = BTreeMap::new();
    let mut root_origin = None;
    let mut footer = None;
    for row in rows {
        let row = row?;
        assert!(footer.is_none(), "no companion rows follow the footer");
        match text(&row, "type")? {
            "source" => {
                counts[0] += 1;
                let physical = row.get("source").ok_or("physical source facts")?;
                if physical
                    .pointer("/path/value")
                    .and_then(Value::as_str)
                    .is_some_and(|path| {
                        path.replace('\\', "/")
                            .eq_ignore_ascii_case(expected_source)
                    })
                {
                    assert!(
                        source.replace(text(physical, "id")?.to_owned()).is_none(),
                        "one matching physical source"
                    );
                }
            }
            "sourceUse" => {
                counts[1] += 1;
                let usage = row.get("sourceUse").ok_or("source use facts")?;
                assert!(
                    uses.insert(text(usage, "id")?.to_owned(), usage.clone())
                        .is_none(),
                    "unique source-use references"
                );
            }
            "field" => {
                counts[2] += 1;
                if text(&row, "path")? == "/authored/root/name" {
                    assert!(
                        root_origin.replace(row).is_none(),
                        "one origin for the authored root name"
                    );
                }
            }
            "end" => footer = Some(row),
            _ => return Err("unexpected material companion row type".into()),
        }
    }
    let footer = footer.ok_or("complete material companion footer")?;
    check_footer(&footer, counts);
    check_root_origin(
        &root_origin.ok_or("authored root name receipt")?,
        &uses,
        &source.ok_or("matching original material Entry")?,
    )
}

fn check_footer(footer: &Value, counts: [u64; 3]) {
    for (key, count) in ["sources", "uses", "fields"].into_iter().zip(counts) {
        assert_eq!(
            footer.get(key).and_then(Value::as_u64),
            Some(count),
            "footer accounts for every {key} row"
        );
    }
}

fn check_root_origin(
    origin: &Value,
    uses: &BTreeMap<String, Value>,
    expected_source: &str,
) -> Checked {
    assert_eq!(
        origin
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(false),
        "the fixture source is unencrypted"
    );
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "the original name has complete source evidence"
    );
    let refs = array(origin, "sourceUsesRefs")?;
    let [binding] = refs else {
        return Err("one original name contributor".into());
    };
    let usage = uses
        .get(binding.as_str().ok_or("material origin reference")?)
        .ok_or("known material source use")?;
    assert_eq!(
        text(usage, "source")?,
        expected_source,
        "the name links to its actual physical Archive Entry"
    );
    assert_eq!(
        text(usage, "scope")?,
        "payload",
        "the original name is a payload fact"
    );
    assert_eq!(
        text(usage, "role")?,
        "payload",
        "the retained field uses the original name"
    );
    assert_eq!(
        usage.pointer("/location/namespace").and_then(Value::as_str),
        Some("materialXmlElement"),
        "the source use names a material XML element"
    );
    assert_eq!(
        usage.pointer("/location/field").and_then(Value::as_str),
        Some("qualifiedName"),
        "the source use addresses its actual XML name"
    );
    Ok(())
}

fn check_receipt(root: &Path, manifest: &Value, paths: &[&str]) -> Checked {
    let receipt: Value = serde_json::from_reader(File::open(root.join("receipt.json"))?)?;
    assert_eq!(
        receipt.get("schema").and_then(Value::as_u64),
        Some(1),
        "package receipt version"
    );
    for field in ["name", "version"] {
        assert_eq!(
            receipt
                .get("generator")
                .and_then(|generator| generator.get(field)),
            manifest
                .get("generator")
                .and_then(|generator| generator.get(field)),
            "receipt and scene name the same exporter {field}"
        );
    }
    let files = array(&receipt, "files")?;
    assert_eq!(
        files.len(),
        paths.len(),
        "the receipt inventories all expected payloads"
    );
    for (file, path) in files.iter().zip(paths) {
        assert_eq!(
            text(file, "path")?,
            *path,
            "receipt order and exact payload identity"
        );
        let digest = digest(&root.join(path))?;
        assert_eq!(
            text(file, "sha256")?,
            digest,
            "actual bytes of {path} match the receipt"
        );
    }
    assert_eq!(
        count_files(root)?,
        paths.len() + 1,
        "on-disk count includes the receipt and no unlisted files"
    );
    Ok(())
}

fn digest(path: &Path) -> Checked<String> {
    use std::fmt::Write as _;

    let bytes = Sha256::digest(fs::read(path)?);
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}")?;
    }
    Ok(output)
}

fn count_files(path: &Path) -> io::Result<usize> {
    let mut count = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            count += count_files(&entry.path())?;
        } else if kind.is_file() {
            count += 1;
        } else {
            return Err(io::Error::other("unexpected non-file in synthetic package"));
        }
    }
    Ok(count)
}

fn array<'value>(owner: &'value Value, field: &str) -> Checked<&'value [Value]> {
    owner
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing fixture array {field}").into())
}

fn text<'value>(owner: &'value Value, field: &str) -> Checked<&'value str> {
    owner
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing fixture text {field}").into())
}
