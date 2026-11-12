// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Draws the synthetic ship through the `p4kx ui hologram` command on a native graphics adapter.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{ArchiveBuilder, TestEntry, assembly_entries, hologram_entries, test_key};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

struct Run {
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

fn write_archive(dir: &Path, entries: Vec<TestEntry>) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(path)
}

fn hologram(archive: &Path, out: &Path, extra: &[&str], json: bool) -> Result<Run, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    command
        .args(["ui", "hologram"])
        .arg(archive)
        .arg("TEST_Ship");
    if !cfg!(target_os = "macos") {
        command.arg("--software");
    }
    let output = command.arg("--out").arg(out).args(extra).output()?;
    Ok(Run {
        stdout: String::from_utf8(output.stdout)?,
        stderr: String::from_utf8(output.stderr)?,
        code: output.status.code(),
    })
}

fn no_adapter(run: &Run) -> bool {
    run.stderr.contains("no graphics adapter")
}

fn skip(test: &str, run: &Run) -> bool {
    if !no_adapter(run) {
        return false;
    }
    assert!(
        !p4kx_testkit::render_adapter_required(),
        "{test}: a graphics adapter is required: {}",
        run.stderr
    );
    #[expect(
        clippy::print_stderr,
        reason = "tells the test run why a test checked nothing"
    )]
    {
        eprintln!("{test}: no graphics adapter, nothing checked");
    }
    true
}

type Decoded = (u32, u32, Vec<[u8; 4]>);

fn pixels(path: &Path) -> Result<Decoded, Box<dyn Error>> {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    let mut reader = decoder.read_info()?;
    let mut buffer = vec![0; reader.output_buffer_size().ok_or("too large")?];
    let frame = reader.next_frame(&mut buffer)?;
    buffer.truncate(frame.buffer_size());
    assert_eq!(frame.color_type, png::ColorType::Rgba, "RGBA");
    Ok((
        frame.width,
        frame.height,
        buffer.as_chunks::<4>().0.to_vec(),
    ))
}

#[test]
fn the_ship_is_drawn_as_a_tinted_shell_in_the_wanted_size() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), assembly_entries())?;
    let out = dir.path().join("ship.png");

    let run = hologram(&archive, &out, &["--size", "160x90"], false)?;

    if skip("the_ship_is_drawn", &run) {
        return Ok(());
    }
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let (width, height, pixels) = pixels(&out)?;
    assert_eq!((width, height), (160, 90), "size");
    assert_eq!(pixels.first(), Some(&[0, 0, 0, 0]), "transparent corner");
    let covered: Vec<_> = pixels.iter().filter(|pixel| pixel[3] > 0).collect();
    assert!(covered.len() > 200, "the shell covers pixels");
    assert!(
        covered
            .iter()
            .filter(|[red, _, blue, _]| blue > red)
            .count()
            * 10
            > covered.len() * 9,
        "tinted blue"
    );
    for expected in [
        "160 x 90 px",
        "6 parts (4 with models",
        "camera from defaults",
    ] {
        assert!(run.stdout.contains(expected), "{expected}: {}", run.stdout);
    }
    Ok(())
}

#[test]
fn the_camera_comes_from_the_record_and_the_options_override_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), hologram_entries([14.0, 45.0, 2.5]))?;
    let out = dir.path().join("ship.png");

    let run = hologram(&archive, &out, &["--size", "64", "--fov", "20"], true)?;

    if skip("the_camera_comes_from_the_record", &run) {
        return Ok(());
    }
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let report: Value = serde_json::from_str(&run.stdout)?;
    let camera = &report["camera"];
    assert_eq!(camera["source"], "record and options", "{camera}");
    assert_eq!(camera["fov_degrees"], 20.0, "the option wins");
    assert_eq!(camera["view_angle_degrees"], 45.0, "the Record");
    assert_eq!(camera["distance_scaler"], 2.5, "the Record");
    assert_eq!(report["width"], 64, "a square");
    assert_eq!(report["height"], 64, "a square");
    assert!(report["bytes"].as_u64().unwrap_or(0) > 0, "{report}");
    assert!(report["milliseconds"]["total"].is_u64(), "timed");
    Ok(())
}

#[test]
fn an_opaque_background_fills_the_picture() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), assembly_entries())?;
    let out = dir.path().join("ship.png");

    let run = hologram(
        &archive,
        &out,
        &["--size", "64x48", "--background", "#102030"],
        false,
    )?;

    if skip("an_opaque_background", &run) {
        return Ok(());
    }
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let (_, _, pixels) = pixels(&out)?;
    assert_eq!(
        pixels.first(),
        Some(&[0x10, 0x20, 0x30, 0xff]),
        "the colour"
    );
    assert!(pixels.iter().all(|pixel| pixel[3] == 255), "opaque");
    Ok(())
}

#[test]
fn bad_options_and_unknown_entities_are_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), assembly_entries())?;
    let out = dir.path().join("ship.png");

    for extra in [
        &["--size", "0x10"][..],
        &["--size", "99999"],
        &["--background", "red"],
        &["--fov", "0"],
        &["--opacity", "2"],
        &["--tint", "#12"],
    ] {
        let run = hologram(&archive, &out, extra, false)?;
        assert_eq!(run.code, Some(2), "{extra:?}: {}", run.stderr);
    }
    let mut command = p4kx_testkit::p4kx()?;
    let output = command
        .args(["ui", "hologram"])
        .arg(&archive)
        .arg("NO_SUCH_SHIP")
        .arg("--out")
        .arg(&out)
        .output()?;
    assert_eq!(output.status.code(), Some(2), "no such Entity");
    assert!(!out.exists(), "nothing is written");
    Ok(())
}

#[test]
fn the_output_cannot_be_the_archive() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), assembly_entries())?;

    let run = hologram(&archive, &archive, &[], false)?;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(run.stderr.contains("is the Archive"), "{}", run.stderr);
    Ok(())
}

#[test]
fn an_output_inside_a_star_citizen_installation_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let channel = dir.path().join("StarCitizen").join("LIVE");
    std::fs::create_dir_all(channel.join("Bin64"))?;
    let archive = write_archive(&channel, assembly_entries())?;
    let out = channel.join("Bin64").join("ship.png");

    let run = hologram(&archive, &out, &[], false)?;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(
        run.stderr.contains("Star Citizen installation"),
        "{}",
        run.stderr
    );
    assert!(!out.exists(), "nothing is written into the installation");
    Ok(())
}

#[test]
fn an_equal_camera_option_retains_tool_origin_with_an_encrypted_config() -> TestResult {
    let dir = tempfile::tempdir()?;
    let entries = hologram_entries([14.0, 45.0, 2.5])
        .into_iter()
        .map(|entry| {
            if entry.path() == "Data\\Game2.dcb" {
                entry.encrypted()
            } else {
                entry
            }
        })
        .collect();
    let archive = write_archive(dir.path(), entries)?;
    let out = dir.path().join("ship.png");
    let key = test_key().to_hex();
    let run = hologram(
        &archive,
        &out,
        &["--size", "64", "--fov", "14", "--key", &key],
        true,
    )?;
    if skip("an_equal_camera_option_retains_tool_origin", &run) {
        return Ok(());
    }
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let report: Value = serde_json::from_str(&run.stdout)?;
    assert_eq!(report["camera"]["fov_degrees"], 14.0);
    let fields = &report["fieldOrigins"];
    for (path, expected) in [
        ("/camera/fov_degrees", false),
        ("/camera/elevation_degrees", false),
        ("/camera/view_angle_degrees", true),
        ("/camera/distance_scaler", true),
        ("/entity", true),
    ] {
        let encrypted = fields
            .get(path)
            .and_then(|origin| origin.get("containsEncryptedSource"))
            .and_then(Value::as_bool);
        assert_eq!(encrypted, Some(expected), "{path}");
    }
    assert_eq!(
        fields["/camera/fov_degrees"]["sourceUsesRefs"],
        serde_json::json!([]),
    );
    let uses = report["provenance"]["uses"]
        .as_array()
        .ok_or("original uses")?;
    assert!(
        uses.iter()
            .any(|usage| usage["location"]["field"] == "ownDefaultViewAngle")
    );
    assert!(
        !uses
            .iter()
            .any(|usage| usage["location"]["field"] == "cameraFOV")
    );
    Ok(())
}
