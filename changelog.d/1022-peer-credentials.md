### Fixed

- Flatpak session authentication accepts a valid same-user peer whose PID is outside the
  application's PID namespace, while retaining the kernel UID check and bounded D-Bus handshake.
