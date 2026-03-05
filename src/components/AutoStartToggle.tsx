/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { ToggleSwitch } from './ui';

interface AutoStartToggleProps {
  enabled: boolean;
  onToggle: () => Promise<void>;
}

export function AutoStartToggle({ enabled, onToggle }: AutoStartToggleProps) {
  return (
    <ToggleSwitch
      enabled={enabled}
      onChange={onToggle}
      label="开机自启动"
      description="系统启动时自动运行图传伴侣"
    />
  );
}
