// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::{
    Availability, Capability, ExportOptions, MediaError, MediaKind, MediaWriter, Projection,
    Provider, ProviderRegistry, SourceContext,
};

use super::support::{self, TestResult};

struct DeclaredProvider(Capability);

impl Provider for DeclaredProvider {
    fn capability(&self) -> Capability {
        self.0.clone()
    }

    fn source(
        &self,
        _source: &SourceContext<'_>,
        _writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        Err(MediaError::Contract(
            "unavailable provider was invoked".into(),
        ))
    }
}

#[test]
fn a_registered_unavailable_provider_is_not_an_empty_success() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let mut capability = Capability::new(
        MediaKind::Audio,
        "synthetic.unavailable",
        "1.0.0",
        "no decoded projection",
    );
    capability.availability = Availability::Unavailable {
        reason: "decoder dependency absent".into(),
    };
    let registry = ProviderRegistry::try_new(vec![Box::new(DeclaredProvider(capability))])?;
    for (name, kinds) in [("explicit", vec![MediaKind::Audio]), ("all", Vec::new())] {
        let output = folder.path().join(name);
        let options = ExportOptions {
            kinds,
            ..ExportOptions::default()
        };
        assert!(
            matches!(
                p4kx_media::export_with(&archive, &output, &options, &registry),
                Err(MediaError::Unsupported {
                    kind: MediaKind::Audio,
                    ..
                })
            ),
            "unavailable registered provider must refuse {name} selection"
        );
        assert!(
            !output.exists(),
            "availability refusal precedes destination creation"
        );
    }
    Ok(())
}

#[test]
fn malformed_provider_versions_are_refused_at_registration() {
    for version in ["", "latest", "1.2", "01.2.3"] {
        let capability = Capability::new(
            MediaKind::Audio,
            "synthetic.version",
            version,
            "explicit preservation scope",
        );
        assert!(
            ProviderRegistry::try_new(vec![Box::new(DeclaredProvider(capability))]).is_err(),
            "{version:?} is not a semantic provider version"
        );
    }
}
