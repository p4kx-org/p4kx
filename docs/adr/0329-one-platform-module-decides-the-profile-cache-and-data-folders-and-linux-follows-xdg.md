# One platform module decides the profile, cache and data folders, and Linux follows XDG

Decided for issue #1015, part of #827 (full Linux compatibility). Builds on ADR-0070 (profile folder
`P4KX` and its migration) and ADR-0027 (the Archive key comes from the user).

## Decision

- `p4kx_settings::locations` is the only code that names a base folder. `base_folder(kind)` returns
  the config, cache or data base folder of the platform; `profile_folder`, `cache_folder` and
  `data_folder` append the application folder `P4KX`. No other crate calls `dirs` for these folders.
- Windows and macOS use the known folders of the platform through the `dirs` crate, exactly as before:
  the profile stays `%APPDATA%\P4KX` with the ADR-0070 migration, the stored Archive key stays
  `%APPDATA%\P4KX\archive.key`. Nothing moves on Windows.
- Every other Unix follows the XDG Base Directory specification: `$XDG_CONFIG_HOME/P4KX`,
  `$XDG_CACHE_HOME/P4KX`, `$XDG_DATA_HOME/P4KX`. An unset, empty or relative variable is ignored and
  the folder is `~/.config`, `~/.cache` or `~/.local/share` below an absolute home folder; without
  either there is no folder and the callers report that the platform has no config directory.
- The XDG rule is a pure function of `XdgEnvironment` (home folder and the three variables), so the
  tests drive it with environment input and run on every platform. The single platform decision is
  the constant that picks known folders or XDG.
- The application folder is `P4KX` on every platform, as the issue states (`~/.config/P4KX`). There is
  no `P4K-Extractor` folder on Linux, so the ADR-0070 migration finds nothing there and does nothing.
- The stored Archive key file is created with mode 0600 through a temporary file that is created with
  that mode and renamed over the key file, so it is never visible with wider permissions, also when it
  replaces a file that had them. On Windows the file keeps the permissions its folder gives it.
- Caches and data have no user yet; the two functions are the seam for the features that need them.

## Consequences

- A user who sets `XDG_CONFIG_HOME` moves the profile with it, as on any XDG application.
- The folder resolution of the RSI Launcher (`launcher_log.rs`) and the testkit's key lookup still call
  `dirs` themselves; #1016 and #1023 move them onto this module.
