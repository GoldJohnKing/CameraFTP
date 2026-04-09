import { describe, test, expect } from 'vitest';

describe('usePreviewConfigListener', () => {
  test('module exports usePreviewConfigListener', async () => {
    const mod = await import('../usePreviewConfigListener');
    expect(typeof mod.usePreviewConfigListener).toBe('function');
  });
});
