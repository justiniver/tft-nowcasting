# TFT Meta Scouts

## Project Overview

**TFT Meta Scouts** is a system for identifying players who consistently discover strong Teamfight Tactics compositions before those compositions become widely adopted.

Most TFT analytics tools describe the present. They show which compositions are popular, which units have the highest placement rates, and what the current meta looks like.

This project asks a different question:

> Can we predict the next meta by finding the players who are repeatedly ahead of it?

Using historical match data from the Riot Games API, the system tracks how compositions spread through high-ranked player populations. It then assigns players an **ahead-of-the-curve score** based on whether they adopt successful strategies before the rest of the ladder.

The recent behavior of these players becomes a predictive signal for emerging compositions.

---

## Core Hypothesis

Some TFT players are consistently better than others at identifying strong strategies early.

They may:

* Recognize underused unit combinations
* Adapt quickly after balance changes
* Discover unusual item or augment interactions
* Import strategies from another region
* Refine experimental compositions before they become standardized

If this behavior persists across multiple patches, these players may function as leading indicators of future meta changes.

The project tests whether following these players produces better forecasts than simply looking at current composition popularity or average placement.

---

## Main Research Question

> Do historically early and successful adopters provide useful information about which TFT compositions will become meta?

A secondary question is whether meta discovery is a repeatable player skill or mostly the result of luck, copying, and temporary experimentation.

---

## Available Data

The Riot TFT API provides completed match data for ranked players.

Each match includes information such as:

* Match timestamp and game version
* Final placement
* Final units and star levels
* Equipped items
* Active traits
* Selected augments
* Player level
* Ranked ladder information

The API does not expose every decision made during the game. The project therefore studies players through their final boards and repeated composition choices rather than attempting to reconstruct every roll, purchase, or pivot.

---

## Composition Discovery

The first challenge is determining what composition each player used.

Instead of relying entirely on manually defined composition labels, the project can represent each final board using features such as:

* Champions
* Traits
* Carry units
* Item combinations
* Augments
* Unit star levels

Similar boards can then be grouped into composition families.

This allows the system to detect new or unusual variants that may not yet have a recognized community name.

---

## Ahead-of-the-Curve Score

A player should not receive credit merely for being the first person to play an unusual board.

A useful discovery should satisfy several conditions:

1. The composition was uncommon when the player used it.
2. The player achieved strong results with it.
3. The composition later became more popular or more successful.
4. The player demonstrated similar behavior across multiple patches.

The score could consider:

* How early the player adopted the composition
* How rare the composition was at the time
* The player’s performance with the composition
* The composition’s later growth in popularity
* Whether the pattern repeats across patches
* Whether other strong players adopted the strategy afterward

This produces a measure of **persistent discovery skill**, rather than rewarding isolated lucky games.

---

## Forecasting the Next Meta

Once predictive players have been identified, their recent behavior can be used to forecast emerging strategies.

For each composition, the system might estimate:

* Probability of becoming widely played
* Expected future usage rate
* Expected future average placement
* Strength of the early-adopter signal
* Number of historically predictive players using it
* Whether adoption is concentrated in one region or spreading globally

A forecast might look like this:

```text
Composition: Experimental Sorcerer Variant

Current usage: 0.9%
Projected usage in 72 hours: 6.4%
Probability of becoming meta: 74%

Leading signal:
8 historically predictive players adopted the composition
across Korea and North America during the past 12 hours.
```

---

## Historical Evaluation

The system should be evaluated through historical replay.

For each past patch, the project would move through matches chronologically and only use information that would have been available at that moment.

At each point in time, it would:

1. Identify the current ahead-of-the-curve players.
2. Observe which compositions they recently adopted.
3. Forecast which compositions would grow over the next several days.
4. Compare the forecast with what actually happened.

This avoids accidentally using future information and creates a realistic out-of-sample test.

---

## Baselines

The player-based signal should be compared against simpler forecasting methods:

* Current composition usage
* Current average placement
* Recent growth in usage
* Challenger-wide trends
* Previous-patch strength
* Random player subsets

The project is successful if the early-adopter model predicts future meta changes earlier or more accurately than these baselines.

---

## Quantitative Themes

Although the subject is a video game, the project draws from several quantitative areas:

* Lead-lag analysis
* Time-series forecasting
* Bayesian updating
* Reputation scoring
* Network diffusion
* Survival and adoption modeling
* Clustering
* Monte Carlo simulation
* Regime-change detection

The player population can be viewed as an information network in which strategies are discovered, tested, copied, refined, and eventually absorbed into the broader meta.

---

## Why Rust

Rust is not being used simply to call an API.

The project can involve large historical datasets and repeated calculations across:

* Thousands of players
* Millions of player-match observations
* Many patches and regions
* Numerous composition definitions
* Multiple forecasting windows
* Bootstrap and Monte Carlo experiments

Rust is particularly well suited for:

* Parallel historical replay
* Multithreaded composition analysis
* High-throughput data processing
* Efficient simulation
* Reliable long-running ingestion pipelines
* Memory-efficient storage of large match histories

The project therefore provides a practical reason to learn Rust while still leaving room for experimentation with systems programming, concurrency, and performance optimization.

---

## Possible Final Product

The final product could be a research engine with a dashboard showing:

* Current emerging compositions
* Players with the strongest historical discovery records
* Regional adoption patterns
* Composition popularity trajectories
* Confidence intervals around forecasts
* Historical forecast accuracy
* A timeline showing how strategies spread through the player network

The dashboard would not simply report the strongest compositions today. It would try to identify what the strongest players are quietly experimenting with before the rest of the ladder notices.

---

## Project Identity

**Name:** TFT Meta Scouts

**One-line description:**

> A Rust-based forecasting engine that identifies players who repeatedly discover strong TFT compositions early and uses their behavior to predict the next meta.

**Core research claim:**

> The next TFT meta may be visible in the behavior of a small group of consistently early adopters before it appears in aggregate statistics.
