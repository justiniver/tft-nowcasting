# TFT Meta Scouts

A Rust project for studying which high-ranked Teamfight Tactics players adopt
successful compositions before those compositions become widely played.

## Running locally

Run the in-memory composition-analysis example:

```bash
cargo run
```

Run a live Riot API smoke test:

```bash
cargo run -- api-smoke
```

Collect a small cached dataset from three Challenger players:

```bash
cargo run -- ingest
```

Optionally provide the number of players and recent matches per player:

```bash
cargo run -- ingest 10 10
```

Audit the cached dataset without making network requests:

```bash
cargo run -- audit
```

Analyze only standard ranked matches from the local cache:

```bash
cargo run -- analyze
```

Ingestion saves original ladder and match JSON under `data/raw/`. Match files
are reused on later runs instead of being downloaded again. Analysis keeps
queue 1100 matches, excludes other modes such as Double Up, and groups boards
that share at least 80% of the larger board's champions. The analysis reports
each family's usage share and its change from the previous populated window.
Emerging candidates must be growing, have at least two plays, and average a
placement of 4.5 or better in the latest window. For each candidate, the report
also identifies players who used that family in the previous populated window.
A historical replay ranks players by repeated successful early-adoption signals,
patch coverage, and their average placement in those early games.

The smoke test reads `RIOT_KEY` from `.env`, verifies authentication, fetches a
Challenger player, retrieves one recent match ID, and converts that match into
the project's observation model. The key is sent only through the
`X-Riot-Token` HTTPS header and is never printed.

The default routes are `jp1` for platform endpoints and `asia` for match
history. They can be overridden with `RIOT_PLATFORM` and `RIOT_REGION`.
