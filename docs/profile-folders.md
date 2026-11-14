# Profile, cache and data folders

Without an override, Windows uses its known folders and Linux follows the XDG base directory
variables. macOS uses `~/Library/Application Support/P4KX` for settings, the stored Archive key
and application data, and `~/Library/Caches/P4KX` for the cache.
Changing `APPDATA` or `USERPROFILE` does not replace Windows known-folder resolution.

Native Linux uses the following directories. An unset, empty or relative XDG variable
uses the home-directory default.

| Content | XDG location | Default |
| --- | --- | --- |
| Settings and stored Archive key | `$XDG_CONFIG_HOME/P4KX` | `~/.config/P4KX` |
| Cache | `$XDG_CACHE_HOME/P4KX` | `~/.cache/P4KX` |
| Application data | `$XDG_DATA_HOME/P4KX` | `~/.local/share/P4KX` |

The settings and key files are `settings.toml` and `archive.key`. Flatpak sets the XDG
directories for its own application profile. [Linux packages](linux-packages.md) covers
the profile, Archive read grants and selected export directories.

Set `P4KX_FOLDERS_ROOT` to an absolute directory before starting P4KX to isolate its application
folders on any supported OS and CPU:

| Content | Location below the explicit root |
| --- | --- |
| Settings | `config/P4KX/settings.toml` |
| Stored Archive key | `config/P4KX/archive.key` |
| Cache | `cache/P4KX` |
| Application data | `data/P4KX` |

The override takes precedence over the platform defaults. An explicitly
empty or relative root provides no application folders; it never falls back to the real user
profile. Unset the variable to restore normal platform defaults. The process fixes its profile
location on first use, so set the variable before launching it.

The override does not move an existing profile, change the installation directory or redirect
explicitly selected export destinations. Old-profile migration searches only the selected config
base. An explicitly configured `P4KX_KEY_FILE` still selects that separate key file, and an
explicit Archive-key variable remains independent of the profile.

Native GUI and package smoke tests set this root and remove inherited Archive-key variables.
Fresh-process tests check Unicode paths, actual settings writes and invalid-root refusal without
mutating the parent process environment. The GUI smoke starts the real application executable;
the root is the production folder-resolution path, rather than a test-only substitute.
