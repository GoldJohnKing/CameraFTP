#!/usr/bin/env node

/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 *
 * Gallery SLO gate validation script.
 * Reads a JSON file of SloSample records, groups by deviceTier × cacheMode,
 * and checks SLO thresholds per bucket.
 *
 * Usage: node scripts/perf/validate-gallery-slo.mjs <samples.json>
 */

import { readFileSync } from 'node:fs';
import { basename } from 'node:path';

const REQUIRED_FIELDS = [
  'galleryOpenStart',
  'galleryFirstInteractive',
  'visibleThumbsExpected',
  'visibleThumbsReady',
  'scrollStop',
  'viewportFullyFilled',
  'deviceTier',
  'cacheMode',
];

const TIERS = ['low', 'mid', 'high'];
const CACHE_MODES = ['cold', 'hot'];

const MIN_SAMPLES = 200;
const MAX_INVALID_RATE = 0.02;
const TTI_P95_LIMIT_MS = 500;
const MIN_FILL_RATE = 0.95;
const FILL_DELAY_P95_LIMIT_MS = 300;

function percentile(sorted, p) {
  const idx = Math.ceil((p / 100) * sorted.length) - 1;
  return sorted[Math.max(0, idx)];
}

function finalizeSample(sample) {
  if (!sample.galleryOpenStart || !sample.galleryFirstInteractive) {
    return { valid: false, reason: 'missing_tti_pair' };
  }
  if (sample.visibleThumbsExpected == null || sample.visibleThumbsReady == null) {
    return { valid: false, reason: 'missing_fill_pair' };
  }
  if (!sample.scrollStop || !sample.viewportFullyFilled) {
    return { valid: false, reason: 'missing_scroll_pair' };
  }
  const ttiMs = sample.galleryFirstInteractive - sample.galleryOpenStart;
  const fillRate = sample.visibleThumbsReady / sample.visibleThumbsExpected;
  const fillDelayMs = sample.viewportFullyFilled - sample.scrollStop;
  return { valid: true, reason: null, ttiMs, fillRate, fillDelayMs };
}

function printHelp() {
  console.log(`Usage: ${basename(process.argv[1])} <samples.json>

Gallery SLO gate validation.

Reads a JSON file containing an array of SloSample objects and validates
SLO thresholds per deviceTier × cacheMode bucket.

Thresholds:
  - Minimum samples per bucket:     ${MIN_SAMPLES}
  - Max invalid sample rate:        ${(MAX_INVALID_RATE * 100).toFixed(0)}%
  - TTI P95:                        ≤ ${TTI_P95_LIMIT_MS} ms
  - Fill rate (within 1s):          ≥ ${(MIN_FILL_RATE * 100).toFixed(0)}%
  - Fill delay P95:                 ≤ ${FILL_DELAY_P95_LIMIT_MS} ms

Exit codes:
  0  All buckets pass
  1  One or more buckets fail or input error
  2  Usage / argument error`);
}

function main() {
  const args = process.argv.slice(2);

  if (args.includes('--help') || args.includes('-h')) {
    printHelp();
    process.exit(0);
  }

  if (args.length !== 1) {
    console.error(`Error: expected exactly one argument (path to samples JSON).\n`);
    printHelp();
    process.exit(2);
  }

  const filePath = args[0];
  let samples;
  try {
    const raw = readFileSync(filePath, 'utf-8');
    samples = JSON.parse(raw);
  } catch (err) {
    console.error(`Error: failed to read or parse "${filePath}": ${err.message}`);
    process.exit(1);
  }

  if (!Array.isArray(samples)) {
    console.error('Error: input JSON must be an array of SloSample objects.');
    process.exit(1);
  }

  // Group by bucket key
  const buckets = new Map();
  for (const s of samples) {
    const key = `${s.deviceTier}|${s.cacheMode}`;
    if (!buckets.has(key)) buckets.set(key, []);
    buckets.get(key).push(s);
  }

  let allPassed = true;
  const results = [];

  for (const tier of TIERS) {
    for (const cache of CACHE_MODES) {
      const key = `${tier}|${cache}`;
      const bucketSamples = buckets.get(key) || [];
      const total = bucketSamples.length;

      const bucketResult = {
        bucket: `${tier}/${cache}`,
        total,
        invalid: 0,
        invalidRate: 0,
        ttiP95: null,
        fillRateAvg: null,
        fillDelayP95: null,
        passed: true,
        failures: [],
      };

      if (total === 0) {
        bucketResult.failures.push('no samples');
        bucketResult.passed = false;
        allPassed = false;
        results.push(bucketResult);
        continue;
      }

      // Finalize each sample
      const finalized = bucketSamples.map(finalizeSample);
      const validSamples = finalized.filter((f) => f.valid);
      const invalidCount = finalized.filter((f) => !f.valid).length;

      bucketResult.invalid = invalidCount;
      bucketResult.invalidRate = invalidCount / total;

      if (total < MIN_SAMPLES) {
        bucketResult.failures.push(`insufficient samples: ${total} < ${MIN_SAMPLES}`);
        bucketResult.passed = false;
      }

      if (bucketResult.invalidRate > MAX_INVALID_RATE) {
        bucketResult.failures.push(
          `invalid rate ${(bucketResult.invalidRate * 100).toFixed(1)}% > ${(MAX_INVALID_RATE * 100).toFixed(0)}%`
        );
        bucketResult.passed = false;
      }

      if (validSamples.length > 0) {
        const ttis = validSamples.map((s) => s.ttiMs).sort((a, b) => a - b);
        const ttiP95 = percentile(ttis, 95);
        bucketResult.ttiP95 = ttiP95;

        if (ttiP95 > TTI_P95_LIMIT_MS) {
          bucketResult.failures.push(`TTI P95 ${ttiP95} ms > ${TTI_P95_LIMIT_MS} ms`);
          bucketResult.passed = false;
        }

        const fillRates = validSamples.map((s) => s.fillRate);
        const fillRateAvg = fillRates.reduce((a, b) => a + b, 0) / fillRates.length;
        bucketResult.fillRateAvg = fillRateAvg;

        if (fillRateAvg < MIN_FILL_RATE) {
          bucketResult.failures.push(
            `fill rate ${(fillRateAvg * 100).toFixed(1)}% < ${(MIN_FILL_RATE * 100).toFixed(0)}%`
          );
          bucketResult.passed = false;
        }

        const fillDelays = validSamples.map((s) => s.fillDelayMs).sort((a, b) => a - b);
        const fillDelayP95 = percentile(fillDelays, 95);
        bucketResult.fillDelayP95 = fillDelayP95;

        if (fillDelayP95 > FILL_DELAY_P95_LIMIT_MS) {
          bucketResult.failures.push(`fill delay P95 ${fillDelayP95} ms > ${FILL_DELAY_P95_LIMIT_MS} ms`);
          bucketResult.passed = false;
        }
      }

      if (!bucketResult.passed) allPassed = false;
      results.push(bucketResult);
    }
  }

  // Print report
  console.log('Gallery SLO Gate Validation');
  console.log('='.repeat(60));
  for (const r of results) {
    const status = r.passed ? 'PASS' : 'FAIL';
    console.log(`\n[${status}] ${r.bucket}  (n=${r.total}, invalid=${r.invalid})`);
    if (r.ttiP95 != null) console.log(`  TTI P95:       ${r.ttiP95.toFixed(1)} ms`);
    if (r.fillRateAvg != null) console.log(`  Fill rate avg: ${(r.fillRateAvg * 100).toFixed(1)}%`);
    if (r.fillDelayP95 != null) console.log(`  Fill delay P95: ${r.fillDelayP95.toFixed(1)} ms`);
    for (const f of r.failures) {
      console.log(`  ✗ ${f}`);
    }
  }
  console.log('\n' + '='.repeat(60));
  console.log(allPassed ? 'ALL BUCKETS PASSED' : 'SOME BUCKETS FAILED');

  process.exit(allPassed ? 0 : 1);
}

main();
