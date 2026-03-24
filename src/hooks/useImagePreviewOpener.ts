/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useCallback } from 'react';
import { useDraftConfig } from '../stores/configStore';
import { openImagePreview } from '../services/image-open';

interface OpenImageParams {
  filePath: string;
  allUris?: string[];
  getAllUris?: () => Promise<string[]>;
}

export function useImagePreviewOpener() {
  const draft = useDraftConfig();

  return useCallback(async ({ filePath, allUris, getAllUris }: OpenImageParams) => {
    await openImagePreview({
      filePath,
      allUris,
      getAllUris,
      openMethod: draft?.androidImageViewer?.openMethod,
    });
  }, [draft?.androidImageViewer?.openMethod]);
}
