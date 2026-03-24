# Gallery Thumbnail V2 — Performance Baseline

> **Status**: Template — populate after first benchmark run.

## Environment

| Field | Value |
|-------|-------|
| Date | — |
| Commit | — |
| Device | — |
| Device tier | — |
| OS / Browser | — |

## SLO Thresholds

| Metric | Target |
|--------|--------|
| TTI P95 | ≤ 500 ms |
| Fill rate (within 1 s) | ≥ 95 % |
| Fill delay P95 | ≤ 300 ms |
| Invalid sample rate | ≤ 2 % |
| Min samples per bucket | ≥ 200 |

## Results

### low / cold

| Metric | Value | Pass? |
|--------|-------|-------|
| Samples | — | — |
| TTI P95 | — | — |
| Fill rate avg | — | — |
| Fill delay P95 | — | — |

### low / hot

| Metric | Value | Pass? |
|--------|-------|-------|
| Samples | — | — |
| TTI P95 | — | — |
| Fill rate avg | — | — |
| Fill delay P95 | — | — |

### mid / cold

| Metric | Value | Pass? |
|--------|-------|-------|
| Samples | — | — |
| TTI P95 | — | — |
| Fill rate avg | — | — |
| Fill delay P95 | — | — |

### mid / hot

| Metric | Value | Pass? |
|--------|-------|-------|
| Samples | — | — |
| TTI P95 | — | — |
| Fill rate avg | — | — |
| Fill delay P95 | — | — |

### high / cold

| Metric | Value | Pass? |
|--------|-------|-------|
| Samples | — | — |
| TTI P95 | — | — |
| Fill rate avg | — | — |
| Fill delay P95 | — | — |

### high / hot

| Metric | Value | Pass? |
|--------|-------|-------|
| Samples | — | — |
| TTI P95 | — | — |
| Fill rate avg | — | — |
| Fill delay P95 | — | — |

## Notes

- Fill rate is measured as `visibleThumbsReady / visibleThumbsExpected` within 1 s of gallery open.
- Fill delay is `viewportFullyFilled − scrollStop`.
- TTI is `galleryFirstInteractive − galleryOpenStart`.
