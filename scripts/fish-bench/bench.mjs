export function rowFor({
  id,
  mode,
  status,
  ms = null,
  ttfaMs = null,
  bytes = null,
  error = null,
}) {
  return {
    id,
    mode,
    ok: status >= 200 && status < 300,
    status,
    ms,
    ttfaMs,
    bytes,
    error,
  };
}

export function summarizeRun(rows) {
  return {
    ok: rows.filter((r) => r.ok).length,
    failed: rows.filter((r) => !r.ok).length,
    rows,
  };
}
