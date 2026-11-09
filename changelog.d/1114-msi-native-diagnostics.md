### Changed

- Native Windows MSI release checks retain bounded, streamed phase diagnostics and preserve
  original errors and required cleanup when diagnostic persistence fails. An unconfirmed
  worker completion remains a failed release check.
