/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { Eye, EyeOff, ExternalLink } from 'lucide-react';
import { ToggleSwitch, Select } from './ui';
import { SEEDREAM_MODELS, DEFAULT_SEEDREAM_MODEL } from '../constants/seedream-models';
import { openExternalLink } from '../utils/external-link';
import type { AppConfig } from '../types';

interface AiEditConfigPanelProps {
  config: AppConfig;
  isLoading: boolean;
  disabled?: boolean;
  updateDraft: (updater: (draft: AppConfig) => AppConfig) => void;
}

export function AiEditConfigPanel({
  config,
  isLoading,
  disabled = false,
  updateDraft,
}: AiEditConfigPanelProps) {
  const [showApiKey, setShowApiKey] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const autoResize = useCallback((el: HTMLTextAreaElement | null) => {
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${el.scrollHeight}px`;
  }, []);
  const [apiKeyInput, setApiKeyInput] = useState(
    () => {
      if (config.aiEdit.provider.type !== 'seed-edit') return '';
      return config.aiEdit.provider.apiKey;
    }
  );
  const [promptInput, setPromptInput] = useState(() => config.aiEdit.prompt);

  useEffect(() => {
    const key = config.aiEdit.provider.type === 'seed-edit'
      ? config.aiEdit.provider.apiKey
      : '';
    setApiKeyInput(key);
  }, [config.aiEdit.provider]);

  useEffect(() => {
    setPromptInput(config.aiEdit.prompt);
  }, [config.aiEdit.prompt]);

  const seedEditConfig = config.aiEdit.provider.type === 'seed-edit'
    ? config.aiEdit.provider : null;

  const handleAutoEditToggle = () => {
    updateDraft(d => ({
      ...d,
      aiEdit: {
        ...d.aiEdit,
        autoEdit: !d.aiEdit.autoEdit,
      },
    }));
  };

  const handlePromptBlur = () => {
    if (promptInput === config.aiEdit.prompt) return;
    updateDraft(d => ({
      ...d,
      aiEdit: {
        ...d.aiEdit,
        prompt: promptInput,
      },
    }));
  };

  const handleApiKeyBlur = () => {
    if (!seedEditConfig) return;
    if (apiKeyInput === seedEditConfig.apiKey) return;
    updateDraft(d => ({
      ...d,
      aiEdit: {
        ...d.aiEdit,
        provider: {
          ...d.aiEdit.provider,
          apiKey: apiKeyInput,
        },
      },
    }));
  };

  return (
    <div className="p-4 space-y-6">
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
        <button
          onClick={() => openExternalLink('https://www.volcengine.com/docs/82379/1399008')}
          className="text-sm text-blue-600 hover:text-blue-700 inline-flex items-center gap-0.5 mt-1"
          type="button"
        >
          开通火山引擎模型服务
          <ExternalLink className="w-3 h-3" />
        </button>
      </div>

      {/* 自动触发开关 */}
      <ToggleSwitch
        enabled={config.aiEdit.autoEdit}
        onChange={handleAutoEditToggle}
        label="自动修图"
        description="接收到图片后自动运行 AI 修图"
        disabled={isLoading || disabled}
      />

      {/* 模型 + 提示词 — 仅在自动修图启用时显示 */}
      {config.aiEdit.autoEdit && (
        <>
          {seedEditConfig && (
            <div className="space-y-2">
              <label className="block text-sm font-medium text-gray-700">
                模型
              </label>
              <Select
                value={seedEditConfig.model || DEFAULT_SEEDREAM_MODEL}
                options={SEEDREAM_MODELS}
                onChange={(model) => {
                  updateDraft(d => ({
                    ...d,
                    aiEdit: {
                      ...d.aiEdit,
                      provider: {
                        ...d.aiEdit.provider,
                        model,
                      },
                    },
                  }));
                }}
                disabled={isLoading || disabled}
              />
            </div>
          )}

          <div className="space-y-2">
            <label className="block text-sm font-medium text-gray-700">
              提示词
            </label>
            <textarea
              ref={(el) => {
                (textareaRef as React.MutableRefObject<HTMLTextAreaElement | null>).current = el;
                autoResize(el);
              }}
              value={promptInput}
              onChange={(e) => {
                setPromptInput(e.target.value);
                autoResize(e.target);
              }}
              onBlur={handlePromptBlur}
              placeholder="请输入提示词"
              rows={1}
              disabled={isLoading || disabled}
              className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm bg-white text-gray-700 resize-none overflow-hidden disabled:opacity-50 disabled:cursor-not-allowed"
            />
            {!promptInput.trim() && (
              <p className="text-xs text-red-500">自动修图需要配置提示词才能生效</p>
            )}
          </div>
        </>
      )}
    </div>
  );
}
