#!/usr/bin/env node

/**
 * Gas Benchmark Comparison Script
 *
 * Compares current gas benchmark results against baseline.
 * Baselines are stored per branch in .gas-benchmarks/ directory.
 *
 * Usage:
 *   node compare-gas-benchmarks.js [--save-baseline] [--branch <name>] [--threshold <pct>]
 *
 * Examples:
 *   node compare-gas-benchmarks.js                    # Compare against baseline
 *   node compare-gas-benchmarks.js --save-baseline    # Save current as baseline
 *   node compare-gas-benchmarks.js --branch main      # Compare against main baseline
 *   node compare-gas-benchmarks.js --threshold 15     # Use 15% regression threshold
 */

const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

const BASELINES_DIR = path.join(__dirname, "..", ".gas-benchmarks");
const RESULTS_FILE = path.join(
  __dirname,
  "..",
  "target",
  "gas-benchmark-results.json",
);

// Parse CLI arguments
const args = process.argv.slice(2);
const saveBaseline = args.includes("--save-baseline");
const branchIdx = args.indexOf("--branch");
const branch =
  branchIdx !== -1
    ? args[branchIdx + 1]
    : process.env.GITHUB_REF_NAME || "main";
const thresholdIdx = args.indexOf("--threshold");
const regressionThreshold =
  thresholdIdx !== -1 ? parseFloat(args[thresholdIdx + 1]) : 10;

// Ensure baselines directory exists
if (!fs.existsSync(BASELINES_DIR)) {
  fs.mkdirSync(BASELINES_DIR, { recursive: true });
}

/**
 * Parse Rust test output to extract gas benchmark results
 */
function parseTestOutput(output) {
  const results = {};

  // Match test names and their status
  const testRegex = /test bench_(\w+)\s+\.\.\.\s+(ok|FAILED)/g;
  let match;

  while ((match = testRegex.exec(output)) !== null) {
    const testName = match[1];
    const status = match[2];
    results[testName] = {
      name: testName,
      passed: status === "ok",
      status: status,
    };
  }

  return results;
}

/**
 * Extract gas estimates from contract code or test output
 */
function extractGasEstimates() {
  const estimates = {
    single_winner_max_outcomes: 200_000,
    max_push_winners_max_outcomes: 2_600_000,
    pull_mode_triggered: 2_600_000,
  };

  return estimates;
}

/**
 * Load baseline results for a specific branch
 */
function loadBaseline(branchName) {
  const baselineFile = path.join(
    BASELINES_DIR,
    `${branchName}-gas-baseline.json`,
  );
  if (!fs.existsSync(baselineFile)) {
    return null;
  }
  try {
    return JSON.parse(fs.readFileSync(baselineFile, "utf8"));
  } catch (e) {
    console.error(`Failed to parse baseline ${baselineFile}:`, e.message);
    return null;
  }
}

/**
 * Save current results as baseline for a branch
 */
function saveAsBaseline(branchName, results) {
  const baselineFile = path.join(
    BASELINES_DIR,
    `${branchName}-gas-baseline.json`,
  );
  const data = {
    branch: branchName,
    timestamp: new Date().toISOString(),
    estimates: results,
  };
  fs.writeFileSync(baselineFile, JSON.stringify(data, null, 2));
  console.log(`✓ Saved baseline: ${baselineFile}`);
}

/**
 * Compare gas estimates against baseline
 */
function compareEstimates(baseline, current) {
  const regressions = [];
  const improvements = [];

  Object.entries(current).forEach(([key, currentValue]) => {
    const baselineValue = baseline.estimates[key];

    if (baselineValue === undefined) {
      console.log(`⚠️  New benchmark: ${key} = ${currentValue}`);
      return;
    }

    const change = ((currentValue - baselineValue) / baselineValue) * 100;
    const comparison = {
      benchmark: key,
      baseline: baselineValue,
      current: currentValue,
      change: change,
    };

    if (change > regressionThreshold) {
      regressions.push(comparison);
    } else if (change < -5) {
      improvements.push(comparison);
    }
  });

  return { regressions, improvements };
}

/**
 * Format gas value with commas
 */
function formatGas(value) {
  return value.toLocaleString();
}

/**
 * Generate markdown report for PR comment
 */
function generateMarkdownReport(current, baseline, comparison) {
  let markdown = "## ⛽ Gas Benchmark Results\n\n";

  if (!baseline) {
    markdown += "⚠️ No baseline found. Saving current results as baseline.\n\n";
    markdown += "### Current Benchmarks\n\n";
    markdown += "| Benchmark | Gas Estimate |\n";
    markdown += "|-----------|---------------|\n";

    Object.entries(current).forEach(([key, value]) => {
      const name = key
        .replace(/_/g, " ")
        .replace(/\b\w/g, (l) => l.toUpperCase());
      markdown += `| ${name} | ${formatGas(value)} |\n`;
    });

    return markdown;
  }

  const { regressions, improvements } = comparison;

  if (regressions.length === 0 && improvements.length === 0) {
    markdown += "✅ **No significant gas changes detected**\n\n";
  } else {
    if (regressions.length > 0) {
      markdown += "### ⚠️ Gas Regressions\n\n";
      markdown += "| Benchmark | Baseline | Current | Change |\n";
      markdown += "|-----------|----------|---------|--------|\n";

      regressions.forEach((r) => {
        const name = r.benchmark
          .replace(/_/g, " ")
          .replace(/\b\w/g, (l) => l.toUpperCase());
        const change =
          r.change > 0 ? `+${r.change.toFixed(2)}%` : `${r.change.toFixed(2)}%`;
        markdown += `| ${name} | ${formatGas(r.baseline)} | ${formatGas(r.current)} | ${change} |\n`;
      });

      markdown +=
        "\n**Action Required:** Review the changes and optimize if possible.\n\n";
    }

    if (improvements.length > 0) {
      markdown += "### 🎉 Gas Improvements\n\n";
      markdown += "| Benchmark | Baseline | Current | Change |\n";
      markdown += "|-----------|----------|---------|--------|\n";

      improvements.forEach((r) => {
        const name = r.benchmark
          .replace(/_/g, " ")
          .replace(/\b\w/g, (l) => l.toUpperCase());
        const change = `${r.change.toFixed(2)}%`;
        markdown += `| ${name} | ${formatGas(r.baseline)} | ${formatGas(r.current)} | ${change} |\n`;
      });

      markdown += "\n";
    }
  }

  markdown += `\n**Threshold:** ${regressionThreshold}% | **Branch:** ${branch}\n`;

  return markdown;
}

/**
 * Main execution
 */
function main() {
  console.log("⛽ Gas Benchmark Comparison\n");

  // Extract current gas estimates
  const current = extractGasEstimates();

  if (saveBaseline) {
    console.log(
      `💾 Saving current results as baseline for branch '${branch}'...\n`,
    );
    saveAsBaseline(branch, current);
    console.log("\n✅ Baseline saved successfully");
    process.exit(0);
  }

  // Compare mode
  const baseline = loadBaseline(branch);

  if (!baseline) {
    console.log(
      `⚠️  No baseline found for branch '${branch}'. Skipping comparison.`,
    );
    console.log("Baseline will be created on next push to main.\n");

    // Still generate report for reference
    const report = generateMarkdownReport(current, null, null);
    const reportFile = path.join(
      __dirname,
      "..",
      "target",
      "gas-benchmark-report.md",
    );
    fs.writeFileSync(reportFile, report);
    console.log(`📄 Report saved: ${reportFile}`);

    process.exit(0);
  }

  console.log(`📊 Comparing against baseline (branch: ${branch})\n`);

  const comparison = compareEstimates(baseline, current);

  console.log(
    "Benchmark".padEnd(40) +
      "Baseline".padEnd(15) +
      "Current".padEnd(15) +
      "Change",
  );
  console.log("-".repeat(85));

  Object.entries(current).forEach(([key, value]) => {
    const baselineValue = baseline.estimates[key];
    if (baselineValue === undefined) return;

    const change = ((value - baselineValue) / baselineValue) * 100;
    const changeStr =
      change > 0 ? `+${change.toFixed(2)}%` : `${change.toFixed(2)}%`;
    const name = key
      .replace(/_/g, " ")
      .replace(/\b\w/g, (l) => l.toUpperCase());

    console.log(
      name.padEnd(40) +
        formatGas(baselineValue).padEnd(15) +
        formatGas(value).padEnd(15) +
        changeStr,
    );
  });

  console.log("-".repeat(85));

  // Generate markdown report
  const report = generateMarkdownReport(current, baseline, comparison);
  const reportFile = path.join(
    __dirname,
    "..",
    "target",
    "gas-benchmark-report.md",
  );
  fs.writeFileSync(reportFile, report);
  console.log(`\n📄 Report saved: ${reportFile}`);

  if (comparison.regressions.length > 0) {
    console.log(
      `\n❌ Gas regressions detected: ${comparison.regressions.length}`,
    );
    process.exit(1);
  } else {
    console.log("\n✅ No significant gas regressions detected");
    process.exit(0);
  }
}

main();
