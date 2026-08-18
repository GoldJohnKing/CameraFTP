/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

/**
 * Shape of an option consumed by the generic Select component.
 *
 * Lives in `src/types` (not `components/ui`) so that non-component modules
 * such as `src/constants` can reference it without a components dependency.
 */
export interface SelectOption {
  value: string;
  label: string;
}
