/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { memo, useCallback } from 'react';
import { Sparkles } from 'lucide-react';
import { useConfigStore, useDraftConfig } from '../stores/configStore';
import { Card, CardHeader, ToggleSwitch } from './ui';
import { AiEditConfigPanel } from './AiEditConfigPanel';
import type { AppConfig } from '../types';

const DEFAULT_AI_EDIT_CONFIG = {
  enabled: false,
  autoEdit: true,
  prompt: '',
  provider: { type: 'seed-edit' as const, apiKey: '' },
};

export const AiEditConfigCard = memo(function AiEditConfigCard() {
  const { isLoading, updateDraft } = useConfigStore();
  const draft = useDraftConfig();

  const handleConfigUpdate = useCallback((updater: (draft: AppConfig) => Partial<AppConfig>) => {
    updateDraft(d => {
      const updates = updater(d);
      return { ...d, ...updates };
    });
  }, [updateDraft]);

  return (
    <Card className="overflow-hidden">
      <CardHeader
        title="AI 修图"
        description="使用 AI 自动优化接收到的照片"
        icon={<Sparkles className="w-5 h-5 text-amber-600" />}
        action={
          <ToggleSwitch
            ariaLabel="启用AI修图"
            enabled={draft?.aiEdit?.enabled ?? false}
            onChange={(enabled) => {
              const currentAiEdit = draft?.aiEdit ?? DEFAULT_AI_EDIT_CONFIG;
              updateDraft(d => ({
                ...d,
                aiEdit: {
                  ...currentAiEdit,
                  enabled,
                },
              }));
            }}
            disabled={isLoading}
          />
        }
      />

      {draft?.aiEdit?.enabled && (
        <AiEditConfigPanel
          config={draft}
          isLoading={isLoading}
          onUpdate={handleConfigUpdate}
        />
      )}
    </Card>
  );
});
