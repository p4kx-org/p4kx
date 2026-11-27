### Fixed

- Disk model and UI-image link lookups preserve filesystem failures instead of reporting them
  as missing files; failed directory listings retain their I/O cause when reused.
