# Gas Benchmark Baselines

This directory stores gas benchmark baselines per branch. Baselines are used to detect gas regressions in pull requests before deployment.

## Structure

Baseline files follow the naming convention:

```
{branch}-gas-baseline.json
```

Examples:

- `main-gas-baseline.json` — Gas benchmark baseline for main branch
- `develop-gas-baseline.json` — Gas benchmark baseline for develop branch

## How It Works

### On Main Branch (Push)

1. Gas benchmarks run
2. Results are saved as baseline for the current branch
3. Baseline is committed and pushed to the repository

### On Pull Requests

1. Gas benchmarks run against the PR code
2. Results are compared against the baseline from the target branch (usually `main`)
3. If any benchmark regresses by more than the threshold (default 10%), the check fails
4. A benchmark report is posted to the PR with details

## Regression Thresholds

Default threshold: **10%**

Benchmarks tracked:

- **Single Winner (Max Outcomes)** — Gas estimate for resolving with 1 winner at max outcomes
- **Max Push Winners (Max Outcomes)** — Gas estimate for resolving with MAX_PUSH_PAYOUT_WINNERS winners
- **Pull Mode Triggered** — Gas estimate when pull mode is triggered (>MAX_PUSH_PAYOUT_WINNERS winners)

## Manual Baseline Update

To manually update baselines for a branch:

```bash
cd contracts/predict-iq
node scripts/compare-gas-benchmarks.js --save-baseline --branch main
```

This will save all current gas estimates as baselines for the specified branch.

## Comparing Against a Specific Branch

To compare results against a different branch's baseline:

```bash
cd contracts/predict-iq
node scripts/compare-gas-benchmarks.js --branch develop --threshold 15
```

This compares against the `develop` branch baseline with a 15% threshold.

## Baseline Files

Baseline files are JSON snapshots of gas estimates and include:

- Gas estimates for each benchmark
- Branch name
- Timestamp of when the baseline was created

They are committed to the repository to ensure consistent regression detection across CI runs.

## Gas Estimate Formula

The gas estimate for payout operations follows this formula:

```
gas_estimate = 100_000 + (winner_count * 50_000)
```

Where:

- `100_000` is the base cost
- `winner_count` is the number of winners
- `50_000` is the per-winner cost

### Examples

- 1 winner: 100,000 + (1 × 50,000) = **150,000** instructions
- 10 winners: 100,000 + (10 × 50,000) = **600,000** instructions
- 50 winners (MAX_PUSH_PAYOUT_WINNERS): 100,000 + (50 × 50,000) = **2,600,000** instructions

## CI Integration

Gas benchmarks are run on every PR and compared against the main branch baseline. If a regression is detected:

1. The CI check fails
2. A detailed report is posted to the PR comment
3. The PR cannot be merged until the regression is resolved

This ensures gas efficiency is maintained across all contract changes.
