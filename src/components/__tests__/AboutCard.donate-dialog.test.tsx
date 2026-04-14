/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AboutCard } from '../AboutCard';
import wechatQrCodeSrc from '../../assets/donate-qrcode-wechat.png';
import { useConfigStore } from '../../stores/configStore';
import { setupReactRoot } from '../../test-utils/react-root';

const { getVersionMock, invokeMock } = vi.hoisted(() => ({
  getVersionMock: vi.fn(),
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/app', () => ({
  getVersion: getVersionMock,
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

import { flush } from '../../test-utils/flush';

describe('AboutCard Android donation flow', () => {
  const { getRoot } = setupReactRoot();
  let saveImageToGalleryMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    getVersionMock.mockReset();
    getVersionMock.mockResolvedValue('1.3.1');
    invokeMock.mockReset();

    saveImageToGalleryMock = vi.fn().mockResolvedValue(
      JSON.stringify({ success: true }),
    );

    vi.stubGlobal('PermissionAndroid', {
      saveImageToGallery: saveImageToGalleryMock,
      openExternalLink: vi.fn(),
    });

    useConfigStore.setState((state) => ({
      ...state,
      platform: 'android',
    }));
  });

  it('opens a full-screen WeChat QR dialog instead of saving the QR image locally', async () => {
    await act(async () => {
      getRoot().render(<AboutCard />);
      await flush();
    });

    const donateEntryButton = Array.from(document.querySelectorAll('button')).find(
      (button) => button.textContent?.includes('捐赠渠道'),
    ) as HTMLButtonElement | undefined;
    expect(donateEntryButton).toBeTruthy();

    await act(async () => {
      donateEntryButton?.click();
      await flush();
    });

    const wechatButton = Array.from(document.querySelectorAll('button')).find(
      (button) => button.textContent?.includes('微信支付'),
    ) as HTMLButtonElement | undefined;
    expect(wechatButton).toBeTruthy();

    await act(async () => {
      wechatButton?.click();
      await flush();
    });

    expect(saveImageToGalleryMock).not.toHaveBeenCalled();
    expect(document.body.textContent).toContain('微信收款');
    expect(document.body.textContent).toContain('请先对当前界面截图');

    const qrCodeImage = document.querySelector(
      'img[alt="微信收款码"]',
    ) as HTMLImageElement | null;
    expect(qrCodeImage?.getAttribute('src')).toBe(wechatQrCodeSrc);

    const overlay = document.querySelector(
      '[data-testid="wechat-donate-dialog-overlay"]',
    ) as HTMLDivElement | null;
    expect(overlay).toBeTruthy();
    expect(overlay?.className).toContain('fixed');
    expect(overlay?.className).toContain('inset-0');
  });
});
