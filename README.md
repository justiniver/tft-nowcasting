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

The smoke test reads `RIOT_KEY` from `.env`, verifies authentication, fetches a
Challenger player, retrieves one recent match ID, and converts that match into
the project's observation model. The key is sent only through the
`X-Riot-Token` HTTPS header and is never printed.

The default routes are `jp1` for platform endpoints and `asia` for match
history. They can be overridden with `RIOT_PLATFORM` and `RIOT_REGION`.
