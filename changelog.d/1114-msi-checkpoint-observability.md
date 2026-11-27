### Fixed

- Native Windows MSI checkpoint capture and upload have separate visible runner steps, with bounded progress markers identifying request validation, process observation and file preservation. Their observations remain unproven until the original lifecycle result and cleanup pass.
- The Finish supervisor launch marks request binding, process creation and receipt persistence.
- Mark the passive Finish probes across source binding, waiting, process checks and receipt writes. Exercise Probe 4 while an isolated harmless worker remains active.
