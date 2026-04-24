#!/usr/bin/env node

/**
 * Error Budget Calculator
 * 
 * Calculates SLO compliance and error budget consumption based on metrics.
 * Supports multiple SLOs and generates reports.
 */

const fs = require('fs');
const path = require('path');

// Load SLO configuration
const sloConfig = JSON.parse(
  fs.readFileSync(path.join(__dirname, '../config/slo.json'), 'utf8')
);

/**
 * Calculate error budget for a given SLO
 * @param {Object} slo - SLO configuration
 * @param {Object} metrics - Actual metrics data
 * @returns {Object} Error budget calculation results
 */
function calculateErrorBudget(slo, metrics) {
  const target = slo.target || 100;
  const errorBudgetPercent = slo.error_budget_percent || (100 - target);
  
  // Calculate actual performance
  const actualPerformance = metrics.success_rate || 0;
  const actualErrors = 100 - actualPerformance;
  
  // Calculate error budget consumption
  const errorBudgetConsumed = (actualErrors / errorBudgetPercent) * 100;
  const errorBudgetRemaining = Math.max(0, 100 - errorBudgetConsumed);
  
  // Calculate burn rate (how fast we're consuming the budget)
  const windowDays = parseWindowDays(slo.measurement_window);
  const burnRate = errorBudgetConsumed / windowDays;
  
  // Determine status
  let status = 'healthy';
  let action = 'Normal operations';
  
  if (errorBudgetRemaining <= 0) {
    status = 'emergency';
    action = 'Emergency - rollback recent changes';
  } else if (errorBudgetRemaining <= 10) {
    status = 'critical';
    action = 'Critical - freeze all deployments';
  } else if (errorBudgetRemaining <= 25) {
    status = 'alert';
    action = 'Alert - freeze non-critical deployments';
  } else if (errorBudgetRemaining <= 50) {
    status = 'warning';
    action = 'Warning - review recent changes';
  }
  
  return {
    slo_name: metrics.name,
    target,
    actual_performance: actualPerformance.toFixed(2),
    error_budget_percent: errorBudgetPercent,
    error_budget_consumed: errorBudgetConsumed.toFixed(2),
    error_budget_remaining: errorBudgetRemaining.toFixed(2),
    burn_rate: burnRate.toFixed(2),
    status,
    action,
    measurement_window: slo.measurement_window,
  };
}

/**
 * Parse measurement window string to days
 * @param {string} window - Window string like "30d", "7d"
 * @returns {number} Number of days
 */
function parseWindowDays(window) {
  const match = window.match(/(\d+)d/);
  return match ? parseInt(match[1]) : 30;
}

/**
 * Check burn rate alerts
 * @param {Object} slo - SLO configuration
 * @param {Object} metrics - Metrics data with time windows
 * @returns {Array} Alert conditions
 */
function checkBurnRateAlerts(slo, metrics) {
  const alerts = [];
  const burnRateConfig = sloConfig.burn_rate_alerts;
  
  // Fast burn check (1h/6h windows)
  if (metrics.burn_rate_1h && metrics.burn_rate_1h > burnRateConfig.fast_burn.burn_rate_threshold) {
    alerts.push({
      severity: 'critical',
      type: 'fast_burn',
      message: `Fast burn detected: ${metrics.burn_rate_1h.toFixed(2)}x (threshold: ${burnRateConfig.fast_burn.burn_rate_threshold}x)`,
      window: '1h',
      description: burnRateConfig.fast_burn.description,
    });
  }
  
  // Slow burn check (6h/24h windows)
  if (metrics.burn_rate_6h && metrics.burn_rate_6h > burnRateConfig.slow_burn.burn_rate_threshold) {
    alerts.push({
      severity: 'warning',
      type: 'slow_burn',
      message: `Slow burn detected: ${metrics.burn_rate_6h.toFixed(2)}x (threshold: ${burnRateConfig.slow_burn.burn_rate_threshold}x)`,
      window: '6h',
      description: burnRateConfig.slow_burn.description,
    });
  }
  
  return alerts;
}

/**
 * Generate SLO report
 * @param {Array} results - Array of error budget calculations
 * @returns {string} Formatted report
 */
function generateReport(results) {
  const timestamp = new Date().toISOString();
  
  let report = `
╔════════════════════════════════════════════════════════════════════════════╗
║                         SLO COMPLIANCE REPORT                              ║
║                    Generated: ${timestamp}                    ║
╚════════════════════════════════════════════════════════════════════════════╝

`;

  results.forEach(result => {
    const statusEmoji = {
      healthy: '✅',
      warning: '⚠️',
      alert: '🚨',
      critical: '🔴',
      emergency: '💀',
    }[result.status] || '❓';
    
    report += `
${statusEmoji} ${result.slo_name}
${'─'.repeat(80)}
Target:                 ${result.target}%
Actual Performance:     ${result.actual_performance}%
Error Budget:           ${result.error_budget_percent}%
Budget Consumed:        ${result.error_budget_consumed}%
Budget Remaining:       ${result.error_budget_remaining}%
Burn Rate:              ${result.burn_rate}% per day
Status:                 ${result.status.toUpperCase()}
Action Required:        ${result.action}
Measurement Window:     ${result.measurement_window}

`;
  });
  
  // Summary
  const healthyCount = results.filter(r => r.status === 'healthy').length;
  const warningCount = results.filter(r => r.status === 'warning').length;
  const alertCount = results.filter(r => r.status === 'alert').length;
  const criticalCount = results.filter(r => r.status === 'critical').length;
  const emergencyCount = results.filter(r => r.status === 'emergency').length;
  
  report += `
╔════════════════════════════════════════════════════════════════════════════╗
║                              SUMMARY                                       ║
╚════════════════════════════════════════════════════════════════════════════╝

Total SLOs:             ${results.length}
✅ Healthy:             ${healthyCount}
⚠️  Warning:            ${warningCount}
🚨 Alert:               ${alertCount}
🔴 Critical:            ${criticalCount}
💀 Emergency:           ${emergencyCount}

`;

  return report;
}

/**
 * Main execution
 */
function main() {
  // Example metrics data (in production, this would come from Prometheus/Grafana)
  const exampleMetrics = [
    {
      name: 'API Availability',
      success_rate: 99.95,
      burn_rate_1h: 2.0,
      burn_rate_6h: 1.5,
    },
    {
      name: 'API Latency P95',
      success_rate: 98.5,
      burn_rate_1h: 5.0,
      burn_rate_6h: 3.0,
    },
    {
      name: 'API Latency P99',
      success_rate: 99.2,
      burn_rate_1h: 8.0,
      burn_rate_6h: 4.0,
    },
    {
      name: 'Database Query Latency',
      success_rate: 97.0,
      burn_rate_1h: 12.0,
      burn_rate_6h: 8.0,
    },
    {
      name: 'Cache Availability',
      success_rate: 99.98,
      burn_rate_1h: 1.0,
      burn_rate_6h: 0.5,
    },
  ];
  
  // Calculate error budgets
  const results = [];
  const slos = sloConfig.service_level_objectives;
  
  exampleMetrics.forEach((metrics, index) => {
    const sloKey = Object.keys(slos)[index];
    if (sloKey) {
      const slo = slos[sloKey];
      const result = calculateErrorBudget(slo, metrics);
      results.push(result);
      
      // Check burn rate alerts
      const alerts = checkBurnRateAlerts(slo, metrics);
      if (alerts.length > 0) {
        console.error(`\n🚨 ALERTS for ${metrics.name}:`);
        alerts.forEach(alert => {
          console.error(`  [${alert.severity.toUpperCase()}] ${alert.message}`);
          console.error(`  ${alert.description}`);
        });
      }
    }
  });
  
  // Generate and display report
  const report = generateReport(results);
  console.log(report);
  
  // Save report to file
  const reportPath = path.join(__dirname, '../reports/slo-report.txt');
  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(reportPath, report);
  console.log(`Report saved to: ${reportPath}`);
  
  // Exit with error code if any SLO is in critical/emergency state
  const hasCritical = results.some(r => ['critical', 'emergency'].includes(r.status));
  process.exit(hasCritical ? 1 : 0);
}

// Run if executed directly
if (require.main === module) {
  main();
}

module.exports = {
  calculateErrorBudget,
  checkBurnRateAlerts,
  generateReport,
};
