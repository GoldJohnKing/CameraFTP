/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import type { DeleteImagesResult } from '../types';

export function buildDeleteFailureMessage(result: DeleteImagesResult): string | null {
  if (result.deleted.length > 0 || result.notFound.length > 0 || result.failed.length === 0) {
    return null;
  }

  return `删除失败：${result.failed.length} 张图片未能删除。请在系统弹窗中确认，或检查系统图库删除权限。`;
}
