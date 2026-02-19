/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import type { ReactNode } from 'react';

type ColorVariant = 'blue' | 'green' | 'purple' | 'orange' | 'indigo';

interface IconContainerProps {
  children: ReactNode;
  color?: ColorVariant;
  className?: string;
}

const colorClasses: Record<ColorVariant, string> = {
  blue: 'bg-blue-50',
  green: 'bg-green-50',
  purple: 'bg-purple-50',
  orange: 'bg-orange-50',
  indigo: 'bg-indigo-50',
};

export function IconContainer({ 
  children, 
  color = 'blue', 
  className = '' 
}: IconContainerProps) {
  return (
    <div className={`w-10 h-10 rounded-lg ${colorClasses[color]} flex items-center justify-center ${className}`}>
      {children}
    </div>
  );
}
