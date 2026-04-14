/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState } from 'react';
import { Eye, EyeOff } from 'lucide-react';
import { ToggleSwitch } from './ui';
import type { AppConfig } from '../types';

interface AiEditConfigPanelProps {
  config: AppConfig;
  isLoading: boolean;
  disabled?: boolean;
  onUpdate: (updater: (draft: AppConfig) => Partial<AppConfig>) => void;
}

export function AiEditConfigPanel({
  config,
  isLoading,
  disabled = false,
  onUpdate,
}: AiEditConfigPanelProps) {
  const [showApiKey, setShowApiKey] = useState(false);
  const [apiKeyInput, setApiKeyInput] = useState(
    () => {
      if (config.aiEdit.provider.type !== 'seed-edit') return '';
      return config.aiEdit.provider.apiKey;
    }
  );
  const [promptInput, setPromptInput] = useState(() => config.aiEdit.prompt);

  const seedEditConfig = config.aiEdit.provider.type === 'seed-edit'
    ? config.aiEdit.provider : null;

  const handleAutoEditToggle = () => {
    onUpdate(() => ({
      aiEdit: {
        ...config.aiEdit,
        autoEdit: !config.aiEdit.autoEdit,
      },
    }));
  };

  const handlePromptBlur = () => {
    if (promptInput === config.aiEdit.prompt) return;
    onUpdate(() => ({
      aiEdit: {
        ...config.aiEdit,
        prompt: promptInput,
      },
    }));
  };

  const handleApiKeyBlur = () => {
    if (!seedEditConfig) return;
    if (apiKeyInput === seedEditConfig.apiKey) return;
    onUpdate(() => ({
      aiEdit: {
        ...config.aiEdit,
        provider: {
          ...config.aiEdit.provider,
          apiKey: apiKeyInput,
        },
      },
    }));
  };

  return (
    <div className="p-4 space-y-6">
      {/* 自动触发开关 */}
      <ToggleSwitch
        enabled={config.aiEdit.autoEdit}
        onChange={handleAutoEditToggle}
        label="自动修图"
        description="接收到图片后自动触发 AI 修图"
        disabled={isLoading || disabled}
      />

      {/* 提示词 */}
      <div className="space-y-2">
        <label className="block text-sm font-medium text-gray-700">
          修图提示词
        </label>
        <textarea
          value={promptInput}
          onChange={(e) => setPromptInput(e.target.value)}
          onBlur={handlePromptBlur}
          placeholder="例如：提升画质，使照片更清晰"
          rows={2}
          disabled={isLoading || disabled}
          className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm bg-white text-gray-700 resize-none disabled:opacity-50 disabled:cursor-not-allowed"
        />
        <p className="text-xs text-gray-500">留空使用默认提示词</p>
      </div>

      {/* API Key */}
      <div className="space-y-2">
        <label className="block text-sm font-medium text-gray-700">
          火山引擎 API Key
        </label>
        <div className="relative">
          <input
            type={showApiKey ? 'text' : 'password'}
            value={apiKeyInput}
            onChange={(e) => setApiKeyInput(e.target.value)}
            onBlur={handleApiKeyBlur}
            placeholder="输入火山引擎 API Key"
            disabled={isLoading || disabled}
            className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm bg-white text-gray-700 pr-10 disabled:opacity-50 disabled:cursor-not-allowed"
          />
          <button
            type="button"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => setShowApiKey(!showApiKey)}
            disabled={isLoading || disabled}
            className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 disabled:opacity-50"
          >
            {showApiKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
          </button>
        </div>
      </div>
    </div>
  );
}
