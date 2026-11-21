// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use p4kx_extraction::Archive;
use p4kx_media::{
    Capability, ExportOptions, MediaError, MediaKind, MediaWriter, Projection, Provider,
    ProviderRegistry, RootSource, SourceContext,
};

use super::support::{self, TestResult};

struct HeldChild(Child);

impl Drop for HeldChild {
    fn drop(&mut self) {
        let _kill = self.0.kill();
        let _wait = self.0.wait();
    }
}

struct BlockingProvider {
    ready: PathBuf,
}

impl Provider for BlockingProvider {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Archive,
            "synthetic.lock",
            "1.0.0",
            "central raw payload",
        )
    }

    fn root(&self, _root: &RootSource, writer: &mut MediaWriter<'_>) -> Result<(), MediaError> {
        fs::write(&self.ready, b"held").map_err(|source| MediaError::Io {
            path: self.ready.clone(),
            source,
        })?;
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            writer.checkpoint()?;
            std::thread::sleep(Duration::from_millis(20));
        }
        Err(MediaError::Contract("subprocess fixture timed out".into()))
    }

    fn source(
        &self,
        _source: &SourceContext<'_>,
        _writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        Ok(Projection::Skipped)
    }
}

#[test]
#[ignore = "subprocess fixture invoked by process lock integration test"]
fn holds_actual_generation_lock() -> TestResult {
    let source = std::env::var_os("P4KX_MEDIA_LOCK_ARCHIVE").ok_or("missing child source")?;
    let output = std::env::var_os("P4KX_MEDIA_LOCK_OUTPUT").ok_or("missing child output")?;
    let ready = std::env::var_os("P4KX_MEDIA_LOCK_READY").ok_or("missing child readiness")?;
    let archive = Archive::open(PathBuf::from(source))?;
    let providers = ProviderRegistry::try_new(vec![Box::new(BlockingProvider {
        ready: ready.into(),
    })])?;
    let options = ExportOptions {
        conflict: p4kx_extraction::ConflictPolicy::Overwrite,
        ..ExportOptions::default()
    };
    p4kx_media::export_with(&archive, &PathBuf::from(output), &options, &providers)?;
    Err("blocking child unexpectedly completed".into())
}

#[test]
fn independent_process_is_excluded_and_crash_releases_kernel_lock() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    p4kx_media::export(&archive, &output, &ExportOptions::default())?;
    let prior = fs::read(output.join("media-index.json"))?;
    let ready = folder.path().join("ready");
    let log = fs::File::create(folder.path().join("child.log"))?;
    let mut child = HeldChild(
        Command::new(std::env::current_exe()?)
            .args([
                "--ignored",
                "--exact",
                "process_lock::holds_actual_generation_lock",
                "--nocapture",
            ])
            .env("P4KX_MEDIA_LOCK_ARCHIVE", archive.path())
            .env("P4KX_MEDIA_LOCK_OUTPUT", &output)
            .env("P4KX_MEDIA_LOCK_READY", &ready)
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()?,
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    while !ready.is_file() && Instant::now() < deadline && child.0.try_wait()?.is_none() {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        ready.is_file(),
        "child never held the actual media generation lock"
    );
    let attempt = p4kx_media::export(&archive, &output, &ExportOptions::default());
    assert!(
        matches!(attempt, Err(MediaError::Io { source, .. }) if source.kind() == std::io::ErrorKind::WouldBlock),
        "another process must be excluded by the kernel lock"
    );
    child.0.kill()?;
    child.0.wait()?;
    assert_eq!(
        fs::read(output.join("media-index.json"))?,
        prior,
        "crash before commit preserves previous index"
    );
    let recovered = p4kx_media::export(&archive, &output, &ExportOptions::default())?;
    assert!(
        matches!(recovered, p4kx_media::ExportOutcome::Skipped { .. }),
        "persistent lock path does not become a stale lock after crash"
    );
    Ok(())
}
