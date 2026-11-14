### Fixed

- Loadout read observers retain original XML Include dependencies as direct operation links,
  including failed child reads and explicit refusals before a read. Cached XML results preserve
  those original dependencies and the current caller without repeatedly replaying a shared graph.
