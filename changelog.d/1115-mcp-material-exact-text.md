### Fixed

- MCP material summaries retain complete names, paths and parameter values within the existing
  response budget. Paging stops before an oversized material and retries that exact item through
  its continuation cursor; a material that cannot fit by itself is explicitly refused.
