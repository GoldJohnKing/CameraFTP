/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { memo } from 'react';
import { Sparkles } from 'lucide-react';
import { useConfigStore, useDraftConfig } from '../stores/configStore';
import { Card, CardHeader } from './ui';
import { AiEditConfigPanel } from './AiEditConfigPanel';

export const AiEditConfigCard = memo(function AiEditConfigCard() {
  const { isLoading, updateDraft } = useConfigStore();
  const draft = useDraftConfig();

  if (!draft) return null;

  return (
    <Card>
      <CardHeader
        title="AI 修图设置"
        description="使用生成式 AI 调整照片"
        icon={<Sparkles className="w-5 h-5 text-amber-600" />}
      />

      <AiEditConfigPanel
        config={draft}
        isLoading={isLoading}
        updateDraft={updateDraft}
      />
    </Card>
  );
});
