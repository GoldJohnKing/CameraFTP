/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import type { SelectOption } from '../components/ui/Select';

export const METERING_MODES: SelectOption[] = [
  { value: 'highlight-safe', label: '高光保护' },
  { value: 'matrix', label: '矩阵测光' },
  { value: 'center-weighted', label: '中央重点测光' },
  { value: 'average', label: '平均测光' },
  { value: 'hybrid', label: '混合测光' },
];
