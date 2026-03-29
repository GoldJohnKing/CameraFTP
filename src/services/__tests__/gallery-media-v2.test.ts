/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it, vi, beforeEach } from 'vitest';
import {
  isGalleryV2Available,
  listMediaPage,
  enqueueThumbnails,
  cancelThumbnailRequests,
  cancelByView,
  registerThumbnailListener,
  unregisterThumbnailListener,
  invalidateMediaIds,
  getQueueStats,
  dispatchThumbnailResult,
} from '../gallery-media-v2';
import type {
  MediaPageRequest,
  MediaPageResponse,
  ThumbRequest,
  ThumbResult,
  QueueStats,
} from '../../types/gallery-v2';

function createMockBridge() {
  return {
    listMediaPage: vi.fn().mockResolvedValue('{}'),
    enqueueThumbnails: vi.fn().mockResolvedValue(''),
    cancelThumbnailRequests: vi.fn().mockResolvedValue(''),
    cancelByView: vi.fn().mockResolvedValue(''),
    registerThumbnailListener: vi.fn().mockResolvedValue(''),
    unregisterThumbnailListener: vi.fn().mockResolvedValue(''),
    invalidateMediaIds: vi.fn().mockResolvedValue(''),
    getQueueStats: vi.fn().mockResolvedValue('{}'),
  };
}

describe('gallery-media-v2 service', () => {
  beforeEach(() => {
    window.GalleryAndroidV2 = undefined;
  });

  describe('isGalleryV2Available', () => {
    it('returns false when bridge is not set', () => {
      expect(isGalleryV2Available()).toBe(false);
    });

    it('returns true when bridge is available', () => {
      window.GalleryAndroidV2 = createMockBridge() as unknown as typeof window.GalleryAndroidV2;
      expect(isGalleryV2Available()).toBe(true);
    });
  });

  describe('listMediaPage', () => {
    it('sends serialized request and parses response', async () => {
      const response: MediaPageResponse = {
        items: [
          {
            mediaId: '1',
            uri: 'content://media/1',
            dateModifiedMs: 1000,
            width: 1920,
            height: 1080,
            mimeType: 'image/jpeg',
            displayName: null,
          },
        ],
        nextCursor: 'cursor-2',
        revisionToken: 'rev-1',
        totalCount: 1,
      };
      const bridge = createMockBridge();
      bridge.listMediaPage.mockResolvedValue(JSON.stringify(response));
      window.GalleryAndroidV2 = bridge as unknown as typeof window.GalleryAndroidV2;

      const req: MediaPageRequest = { cursor: null, pageSize: 20, sort: 'dateDesc' };
      const result = await listMediaPage(req);

      expect(bridge.listMediaPage).toHaveBeenCalledWith(JSON.stringify(req));
      expect(result).toEqual(response);
    });

    it('throws when bridge is unavailable', async () => {
      const req: MediaPageRequest = { cursor: null, pageSize: 20, sort: 'dateDesc' };
      await expect(listMediaPage(req)).rejects.toThrow('GalleryAndroidV2 bridge is not available');
    });
  });

  describe('enqueueThumbnails', () => {
    it('serializes and sends thumbnail requests', async () => {
      const bridge = createMockBridge();
      window.GalleryAndroidV2 = bridge as unknown as typeof window.GalleryAndroidV2;

      const reqs: ThumbRequest[] = [
        {
          requestId: 'r1',
          mediaId: '1',
          uri: 'content://media/1',
          dateModifiedMs: 1000,
          sizeBucket: 's',
          priority: 'visible',
          viewId: 'view-1',
        },
      ];

      await enqueueThumbnails(reqs);
      expect(bridge.enqueueThumbnails).toHaveBeenCalledWith(JSON.stringify(reqs));
    });
  });

  describe('cancelThumbnailRequests', () => {
    it('sends request IDs as JSON array', async () => {
      const bridge = createMockBridge();
      window.GalleryAndroidV2 = bridge as unknown as typeof window.GalleryAndroidV2;

      await cancelThumbnailRequests(['r1', 'r2']);
      expect(bridge.cancelThumbnailRequests).toHaveBeenCalledWith(JSON.stringify(['r1', 'r2']));
    });
  });

  describe('cancelByView', () => {
    it('sends view ID directly', async () => {
      const bridge = createMockBridge();
      window.GalleryAndroidV2 = bridge as unknown as typeof window.GalleryAndroidV2;

      await cancelByView('view-1');
      expect(bridge.cancelByView).toHaveBeenCalledWith('view-1');
    });
  });

  describe('registerThumbnailListener', () => {
    it('registers with bridge and stores listener', async () => {
      const bridge = createMockBridge();
      window.GalleryAndroidV2 = bridge as unknown as typeof window.GalleryAndroidV2;

      const listener = vi.fn();
      await registerThumbnailListener('view-1', 'listener-1', listener);
      expect(bridge.registerThumbnailListener).toHaveBeenCalledWith('view-1', 'listener-1');
    });
  });

  describe('unregisterThumbnailListener', () => {
    it('unregisters from bridge', async () => {
      const bridge = createMockBridge();
      window.GalleryAndroidV2 = bridge as unknown as typeof window.GalleryAndroidV2;

      await unregisterThumbnailListener('listener-1');
      expect(bridge.unregisterThumbnailListener).toHaveBeenCalledWith('listener-1');
    });
  });

  describe('invalidateMediaIds', () => {
    it('serializes media IDs as JSON array', async () => {
      const bridge = createMockBridge();
      window.GalleryAndroidV2 = bridge as unknown as typeof window.GalleryAndroidV2;

      await invalidateMediaIds(['1', '2', '3']);
      expect(bridge.invalidateMediaIds).toHaveBeenCalledWith(JSON.stringify(['1', '2', '3']));
    });
  });

  describe('getQueueStats', () => {
    it('parses queue stats response', async () => {
      const stats: QueueStats = { pending: 5, running: 2, cacheHitRate: 0.85 };
      const bridge = createMockBridge();
      bridge.getQueueStats.mockResolvedValue(JSON.stringify(stats));
      window.GalleryAndroidV2 = bridge as unknown as typeof window.GalleryAndroidV2;

      const result = await getQueueStats();
      expect(result).toEqual(stats);
    });
  });

  describe('dispatchThumbnailResult', () => {
    it('dispatches parsed result to registered listener', async () => {
      const bridge = createMockBridge();
      window.GalleryAndroidV2 = bridge as unknown as typeof window.GalleryAndroidV2;

      const listener = vi.fn();
      await registerThumbnailListener('view-1', 'listener-1', listener);

      const result: ThumbResult = {
        requestId: 'r1',
        mediaId: '1',
        status: 'ready',
        localPath: '/cache/thumb_r1.jpg',
      };

      dispatchThumbnailResult('listener-1', JSON.stringify(result));
      expect(listener).toHaveBeenCalledWith(result);
    });

    it('ignores dispatch for unregistered listener', () => {
      const result: ThumbResult = {
        requestId: 'r1',
        mediaId: '1',
        status: 'failed',
        errorCode: 'io_transient',
      };

      // Should not throw
      expect(() => dispatchThumbnailResult('unknown', JSON.stringify(result))).not.toThrow();
    });
  });

  describe('all 8 bridge methods have adapter functions', () => {
    it('exports all expected functions', () => {
      expect(typeof listMediaPage).toBe('function');
      expect(typeof enqueueThumbnails).toBe('function');
      expect(typeof cancelThumbnailRequests).toBe('function');
      expect(typeof cancelByView).toBe('function');
      expect(typeof registerThumbnailListener).toBe('function');
      expect(typeof unregisterThumbnailListener).toBe('function');
      expect(typeof invalidateMediaIds).toBe('function');
      expect(typeof getQueueStats).toBe('function');
    });
  });

  describe('Promise semantics', () => {
    it('all bridge calls return promises', async () => {
      const bridge = createMockBridge();
      window.GalleryAndroidV2 = bridge as unknown as typeof window.GalleryAndroidV2;

      const req: MediaPageRequest = { cursor: null, pageSize: 10, sort: 'dateDesc' };

      expect(listMediaPage(req)).toBeInstanceOf(Promise);
      expect(enqueueThumbnails([])).toBeInstanceOf(Promise);
      expect(cancelThumbnailRequests([])).toBeInstanceOf(Promise);
      expect(cancelByView('v')).toBeInstanceOf(Promise);
      expect(registerThumbnailListener('v', 'l', vi.fn())).toBeInstanceOf(Promise);
      expect(unregisterThumbnailListener('l')).toBeInstanceOf(Promise);
      expect(invalidateMediaIds([])).toBeInstanceOf(Promise);
      expect(getQueueStats()).toBeInstanceOf(Promise);
    });
  });
});
