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

Backfill the accessible history for a TFT set, starting from the ten highest-LP
Challenger players in the current ladder snapshot:

```bash
cargo run -- backfill 17
```

An optional second argument changes the size of that player cohort. The command
pages backward until each history reaches the previous standard-ranked set,
saves matches as it goes, and reuses cached files when restarted:

```bash
cargo run -- backfill 17 25
```

Audit the cached dataset without making network requests:

```bash
cargo run -- audit
```

Analyze only standard ranked matches from the local cache:

```bash
cargo run -- analyze
```

Ingestion and backfill save original ladder and match JSON under `data/raw/`.
Match files are reused on later runs instead of being downloaded again. Analysis keeps
Set 17 queue 1100 matches, excludes other modes and sets, and groups boards
that share at least 80% of the larger board's champions. The analysis reports
each family's usage share and its change from the previous populated window.
Emerging candidates must be growing, have at least two plays, and average a
placement of 4.5 or better in the latest window. For each candidate, the report
also identifies players who used that family in the previous populated window.
A historical replay ranks players by repeated successful early-adoption signals,
patch coverage, and their average placement in those early games. Players with
at least two signals are established scouts; their latest boards form the
next-window forecast. Historical evaluation compares its top prediction with
simple current-popularity and current-performance baselines.

The smoke test reads `RIOT_KEY` from `.env`, verifies authentication, fetches a
Challenger player, retrieves one recent match ID, and converts that match into
the project's observation model. The key is sent only through the
`X-Riot-Token` HTTPS header and is never printed.

The default routes are `jp1` for platform endpoints and `asia` for match
history. They can be overridden with `RIOT_PLATFORM` and `RIOT_REGION`.
