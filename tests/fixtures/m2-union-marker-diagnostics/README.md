# Pkl 0.32.0 union default marker diagnostics

Forty separate source modules exercise union default markers in valid unions,
invalid single types, nested types, type aliases, declaration contexts, and
first-error ordering. `programs.json` pins each source byte string, ordered
query, and the shared module URI. The archive contains two fresh Java sessions
per module with all raw public and producer channels. The offline verifier is
`tools/union_marker_fixtures.py`.

These Java observations are parser authority for this selected matrix. They do
not imply full type-system, API, platform, or M2 compatibility.
