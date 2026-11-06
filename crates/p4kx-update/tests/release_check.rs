// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Asking a releases endpoint whether a newer Release than the running one exists.

use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread::{self, JoinHandle};

use p4kx_update::{Release, ReleaseCheck};

type TestResult = Result<(), Box<dyn Error>>;

const PAGE: &str = "https://github.com/p4kx-org/p4kx/releases/tag/v0.2.0";

struct Server {
    endpoint: String,
    request: JoinHandle<std::io::Result<Vec<String>>>,
}

impl Server {
    fn answering(status: &'static str, body: String) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = format!(
            "http://{}/repos/p4kx-org/p4kx/releases/latest",
            listener.local_addr()?
        );
        let request = thread::spawn(move || {
            let (stream, _) = listener.accept()?;
            let mut reader = BufReader::new(stream.try_clone()?);
            let mut lines = Vec::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line)?;
                let line = line.trim_end().to_owned();
                if line.is_empty() {
                    break;
                }
                lines.push(line);
            }
            let mut stream = stream;
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )?;
            stream.flush()?;
            Ok(lines)
        });
        Ok(Self { endpoint, request })
    }

    fn release(tag: &str) -> std::io::Result<Self> {
        Self::answering(
            "200 OK",
            format!(r#"{{"tag_name":"{tag}","html_url":"{PAGE}","name":"P4KX","draft":false}}"#),
        )
    }

    fn request(self) -> Result<Vec<String>, Box<dyn Error>> {
        Ok(self
            .request
            .join()
            .map_err(|_| "server thread panicked")??)
    }
}

#[test]
fn a_newer_release_is_reported_with_its_page() -> TestResult {
    let server = Server::release("v0.2.0")?;

    let newer = ReleaseCheck::new(&server.endpoint).newer_than("0.1.0")?;

    assert_eq!(
        newer,
        Some(Release {
            version: "0.2.0".to_owned(),
            page: PAGE.to_owned(),
            asset: None,
        }),
        "the newer Release"
    );
    server.request()?;
    Ok(())
}

#[test]
fn the_running_release_and_older_ones_are_not_reported() -> TestResult {
    for (tag, current) in [
        ("v0.2.0", "0.2.0"),
        ("v0.1.9", "0.2.0"),
        ("0.1.0", "0.10.0"),
    ] {
        let server = Server::release(tag)?;

        let newer = ReleaseCheck::new(&server.endpoint).newer_than(current)?;

        assert_eq!(newer, None, "{tag} is not newer than {current}");
        server.request()?;
    }
    Ok(())
}

#[test]
fn a_release_is_newer_than_its_own_pre_release() -> TestResult {
    let server = Server::release("v0.2.0")?;

    let newer = ReleaseCheck::new(&server.endpoint).newer_than("0.2.0-rc.1")?;

    assert!(newer.is_some(), "0.2.0 follows 0.2.0-rc.1");
    server.request()?;
    Ok(())
}

#[test]
fn the_request_carries_nothing_but_the_get_and_its_basic_headers() -> TestResult {
    let server = Server::release("v0.2.0")?;

    ReleaseCheck::new(&server.endpoint).newer_than("0.1.0")?;

    let request = server.request()?;
    assert_eq!(
        request.first().map(String::as_str),
        Some("GET /repos/p4kx-org/p4kx/releases/latest HTTP/1.1"),
        "a plain GET without a query"
    );
    let names: Vec<String> = request
        .iter()
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .map(|(name, _)| name.to_ascii_lowercase())
        .collect();
    assert!(
        names.iter().all(
            |name| ["host", "user-agent", "accept", "accept-encoding"].contains(&name.as_str())
        ),
        "only basic headers: {names:?}"
    );
    assert!(
        request
            .iter()
            .any(|line| line.to_ascii_lowercase().starts_with("user-agent: p4kx/")),
        "names the application: {request:?}"
    );
    Ok(())
}

#[test]
fn a_failed_request_is_an_error() -> TestResult {
    let server = Server::answering("404 Not Found", r#"{"message":"Not Found"}"#.to_owned())?;

    let result = ReleaseCheck::new(&server.endpoint).newer_than("0.1.0");

    assert!(result.is_err(), "no Release: {result:?}");
    server.request()?;
    Ok(())
}

#[test]
fn an_answer_without_a_version_tag_is_an_error() -> TestResult {
    let server = Server::release("latest")?;

    let result = ReleaseCheck::new(&server.endpoint).newer_than("0.1.0");

    assert!(result.is_err(), "not a version: {result:?}");
    server.request()?;
    Ok(())
}

#[test]
fn the_default_check_asks_the_github_api_of_this_repository() {
    assert_eq!(
        ReleaseCheck::github().endpoint(),
        "https://api.github.com/repos/p4kx-org/p4kx/releases/latest",
        "the latest Release of this repository"
    );
}

#[test]
fn a_package_check_keeps_the_page_and_selects_only_a_compatible_asset() -> TestResult {
    let platform = p4kx_update::Platform::current().ok_or("unsupported test platform")?;
    let row = p4kx_update::supported_targets()?
        .into_iter()
        .find(|row| row.platform() == platform)
        .ok_or("missing native target")?;
    let package = *row.packages.first().ok_or("missing native package")?;
    let name =
        p4kx_update::asset_filename("0.5.0", platform, package)?.ok_or("missing asset name")?;
    let page = "https://github.com/p4kx-org/p4kx/releases/tag/v0.5.0";
    let url = format!("https://github.com/p4kx-org/p4kx/releases/download/v0.5.0/{name}");
    let server = Server::answering(
        "200 OK",
        serde_json::json!({
            "tag_name": "v0.5.0",
            "html_url": page,
            "assets": [{"name": name, "browser_download_url": url}]
        })
        .to_string(),
    )?;
    let release = ReleaseCheck::new(&server.endpoint)
        .with_package(package)
        .newer_than("0.4.2")?
        .ok_or("missing newer release")?;
    assert_eq!(release.page, page);
    assert_eq!(release.link(), url);
    assert_eq!(
        release.asset.as_ref().map(|asset| &asset.target),
        Some(&row.target)
    );
    server.request()?;
    Ok(())
}

#[test]
fn malformed_package_metadata_retains_the_release_page_fallback() -> TestResult {
    let platform = p4kx_update::Platform::current().ok_or("unsupported test platform")?;
    let package = if platform.os == p4kx_update::OperatingSystem::Windows {
        p4kx_update::PackageKind::Msi
    } else {
        p4kx_update::PackageKind::TarGz
    };
    for assets in [
        serde_json::Value::Null,
        serde_json::json!({"malformed":true}),
        serde_json::json!([{"name":17}]),
    ] {
        let server = Server::answering(
            "200 OK",
            serde_json::json!({
                "tag_name": "v0.5.0", "html_url": PAGE, "assets": assets
            })
            .to_string(),
        )?;
        let release = ReleaseCheck::new(&server.endpoint)
            .with_package(package)
            .newer_than("0.4.2")?
            .ok_or("missing newer release")?;
        assert_eq!(release.asset, None);
        assert_eq!(release.link(), PAGE);
        server.request()?;
    }
    Ok(())
}
