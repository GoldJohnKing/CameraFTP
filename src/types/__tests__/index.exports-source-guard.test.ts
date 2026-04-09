/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

describe('types/index.ts exports (source guard)', () => {
  it('omits redundant/dead re-exports', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/types/index.ts'), 'utf-8');
    expect(source).not.toContain("export type { AuthConfig }");
    expect(source).not.toContain("export type { ImageOpenMethod }");
    expect(source).not.toContain("export type { AndroidImageViewerConfig }");
    expect(source).not.toContain("export type { ConfigChangedEvent } from './events';");
    expect(source).not.toContain("ThumbSizeBucket");
    expect(source).not.toContain("ThumbPriority");
    expect(source).not.toContain("ThumbStatus");
    expect(source).not.toContain("ThumbErrorCode");
  });

  it('omits dead gallery-v2 re-exports (consumers import directly from gallery-v2)', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/types/index.ts'), 'utf-8');
    expect(source).not.toContain("MediaPageRequest");
    expect(source).not.toContain("MediaPageResponse");
    expect(source).not.toContain("MediaCursor");
    expect(source).not.toContain("ThumbRequest");
    expect(source).not.toContain("ThumbResult");
    expect(source).not.toContain("ThumbResultListener");
  });
});
