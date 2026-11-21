// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::portal_alias::UnknownAlias;

use super::TestError;

#[derive(Debug)]
pub(super) struct FixtureFailure {
    pub(super) client: Option<UnknownAlias>,
    pub(super) service: Option<TestError>,
    pub(super) close: Option<UnknownAlias>,
}

impl Display for FixtureFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fake bus fixture failed: client={:?}; service={:?}; close={:?}",
            self.client, self.service, self.close
        )
    }
}

impl Error for FixtureFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match (&self.service, &self.close, &self.client) {
            (Some(source), _, _) => Some(source.as_ref()),
            (_, Some(source), _) | (_, _, Some(source)) => Some(source),
            (None, None, None) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io;

    use crate::portal_alias::{AliasUncertainty, UnknownAlias};

    use super::super::{TestError, TestResult};
    use super::FixtureFailure;

    #[test]
    fn simultaneous_failures_keep_each_typed_cause() -> TestResult {
        let error: TestError = Box::new(FixtureFailure {
            client: Some(UnknownAlias::with_source(
                AliasUncertainty::Timeout,
                io::Error::from(io::ErrorKind::TimedOut),
            )),
            service: Some(Box::new(io::Error::from(io::ErrorKind::NotFound))),
            close: Some(UnknownAlias::with_source(
                AliasUncertainty::SessionBus,
                io::Error::from(io::ErrorKind::BrokenPipe),
            )),
        });
        let failure = error
            .downcast_ref::<FixtureFailure>()
            .ok_or("the aggregate retains its concrete type")?;
        let client = failure.client.as_ref().ok_or("client error is retained")?;
        assert_eq!(client.reason(), AliasUncertainty::Timeout);
        assert_eq!(
            client
                .source()
                .and_then(|source| source.downcast_ref::<io::Error>())
                .map(io::Error::kind),
            Some(io::ErrorKind::TimedOut),
        );
        assert_eq!(
            failure
                .service
                .as_ref()
                .and_then(|source| source.downcast_ref::<io::Error>())
                .map(io::Error::kind),
            Some(io::ErrorKind::NotFound),
        );
        let close = failure.close.as_ref().ok_or("close error is retained")?;
        assert_eq!(close.reason(), AliasUncertainty::SessionBus);
        assert_eq!(
            close
                .source()
                .and_then(|source| source.downcast_ref::<io::Error>())
                .map(io::Error::kind),
            Some(io::ErrorKind::BrokenPipe),
        );
        assert_eq!(
            failure
                .source()
                .and_then(|source| source.downcast_ref::<io::Error>())
                .map(io::Error::kind),
            Some(io::ErrorKind::NotFound),
            "the fatal service failure is the canonical source",
        );
        Ok(())
    }

    #[test]
    fn a_close_failure_is_a_source_without_losing_the_client_error() -> TestResult {
        let failure = FixtureFailure {
            client: Some(UnknownAlias::new(AliasUncertainty::Timeout)),
            service: None,
            close: Some(UnknownAlias::with_source(
                AliasUncertainty::SessionBus,
                io::Error::from(io::ErrorKind::BrokenPipe),
            )),
        };
        let close = failure
            .source()
            .and_then(|source| source.downcast_ref::<UnknownAlias>())
            .ok_or("close uncertainty is the canonical source")?;
        assert_eq!(close.reason(), AliasUncertainty::SessionBus);
        assert_eq!(
            close
                .source()
                .and_then(|source| source.downcast_ref::<io::Error>())
                .map(io::Error::kind),
            Some(io::ErrorKind::BrokenPipe),
        );
        assert_eq!(
            failure.client.as_ref().map(UnknownAlias::reason),
            Some(AliasUncertainty::Timeout),
        );
        Ok(())
    }
}
