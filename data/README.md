# Local data

The ingestion command creates untracked runtime data here:

- `raw/ladders/<platform>/<timestamp>.json` contains Riot ladder snapshots.
- `raw/matches/<region>/<match-id>.json` contains original match responses.
- `reports/` is reserved for future generated analysis output.

Raw API responses are the source of truth and should not be edited manually.
Small, sanitized JSON used by tests belongs in `tests/fixtures/` instead.
