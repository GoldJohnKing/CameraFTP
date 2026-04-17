/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

export interface SeedreamModel {
  /** Display name shown in UI */
  label: string;
  /** API model ID sent to Volcengine */
  value: string;
}

export const SEEDREAM_MODELS: SeedreamModel[] = [
  { label: 'Doubao-Seedream-5.0-lite', value: 'doubao-seedream-5-0-260128' },
  { label: 'Doubao-Seedream-4.5', value: 'doubao-seedream-4-5-251128' },
  { label: 'Doubao-Seedream-4.0', value: 'doubao-seedream-4-0-250828' },
];

export const DEFAULT_SEEDREAM_MODEL = 'doubao-seedream-5-0-260128';

export function getSeedreamModelLabel(modelId: string): string {
  return SEEDREAM_MODELS.find(m => m.value === modelId)?.label ?? modelId;
}
