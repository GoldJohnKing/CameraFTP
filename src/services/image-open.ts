/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { invoke } from '@tauri-apps/api/core';
import type { AndroidImageOpenMethod, ExifInfo } from '../types';

interface OpenImagePreviewParams {
  filePath: string;
  openMethod?: AndroidImageOpenMethod;
  preferReuse?: boolean;
  allUris?: string[];
  getAllUris?: () => Promise<string[]>;
}

async function resolveViewerUris(params: {
  filePath: string;
  allUris?: string[];
  getAllUris?: () => Promise<string[]>;
}): Promise<string[]> {
  const { filePath, allUris, getAllUris } = params;
  if (allUris) {
    return allUris.length > 0 ? allUris : [filePath];
  }

  try {
    const resolved = getAllUris ? await getAllUris() : [];
    return resolved.length > 0 ? resolved : [filePath];
  } catch {
    return [filePath];
  }
}

async function sendExifToViewer(path: string): Promise<void> {
  if (!window.ImageViewerAndroid?.onExifResult) {
    return;
  }

  const realPath = window.ImageViewerAndroid.resolveFilePath?.(path) ?? path;
  try {
    const exif = await invoke<ExifInfo | null>('get_image_exif', { filePath: realPath });
    window.ImageViewerAndroid.onExifResult(exif ? JSON.stringify(exif) : null);
  } catch {
    // Keep current behavior: ignore EXIF fetch failures silently.
  }
}

function isChooserOpenSuccess(result: unknown): boolean {
  if (typeof result !== 'string' || result.length === 0) {
    return false;
  }

  try {
    const parsed = JSON.parse(result) as { success?: unknown };
    return parsed.success === true;
  } catch {
    return false;
  }
}

export async function openImagePreview({
  filePath,
  openMethod,
  preferReuse,
  allUris,
  getAllUris,
}: OpenImagePreviewParams): Promise<void> {
  const imageViewerAndroid = window.ImageViewerAndroid;

  if (openMethod === 'built-in-viewer' && imageViewerAndroid) {
    const viewerUris = await resolveViewerUris({ filePath, allUris, getAllUris });
    const viewerUrisJson = JSON.stringify(viewerUris);

    if (preferReuse && imageViewerAndroid.openOrNavigateTo) {
      try {
        if (imageViewerAndroid.openOrNavigateTo(filePath, viewerUrisJson)) {
          void sendExifToViewer(filePath);
          return;
        }
      } catch {
        // Fall through to other open methods when bridge call fails.
      }
    }

    if (imageViewerAndroid.openViewer) {
      try {
        if (imageViewerAndroid.openViewer(filePath, viewerUrisJson)) {
          void sendExifToViewer(filePath);
          return;
        }
      } catch {
        // Fall through to chooser/window fallback when bridge call fails.
      }
    }
  }

  if (window.PermissionAndroid?.openImageWithChooser) {
    try {
      const chooserResult = window.PermissionAndroid.openImageWithChooser(filePath);
      if (isChooserOpenSuccess(chooserResult)) {
        return;
      }
    } catch {
      // Fall through to preview window fallback.
    }
  }

  await invoke('open_preview_window', { filePath });
}
