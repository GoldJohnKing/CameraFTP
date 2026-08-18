/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, useEffect, useRef } from 'react';
import { toast } from 'sonner';
import { Sparkles } from 'lucide-react';
import { Dialog } from './ui/Dialog';
import { ToggleSwitch } from './ui/ToggleSwitch';
import { ApiKeyField } from './ui/ApiKeyField';
import { Select } from './ui/Select';
import { SEEDREAM_MODELS, DEFAULT_SEEDREAM_MODEL } from '../types';
import { formatError } from '../utils/error';

interface PromptDialogProps {
  isOpen: boolean;
  defaultPrompt: string;
  defaultModel?: string;
  autoEditEnabled?: boolean;
  hasApiKey?: boolean;
  onConfirm: (prompt: string, model: string, saveAsAutoEdit: boolean, apiKey?: string) => void | Promise<void>;
  onCancel: () => void;
}

export function PromptDialog({ isOpen, defaultPrompt, defaultModel, autoEditEnabled, hasApiKey = true, onConfirm, onCancel }: PromptDialogProps) {
  const [prompt, setPrompt] = useState(defaultPrompt);
  const [model, setModel] = useState(defaultModel ?? DEFAULT_SEEDREAM_MODEL);
  const [saveAsAutoEdit, setSaveAsAutoEdit] = useState(false);
  const [apiKey, setApiKey] = useState('');
  const [showApiKey, setShowApiKey] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const apiKeyInputRef = useRef<HTMLInputElement>(null);

  const needsApiKey = hasApiKey === false;
  // Keep the latest value for the deferred focus callback without re-running
  // the reset effect (which would wipe user input) when hasApiKey changes.
  const needsApiKeyRef = useRef(needsApiKey);
  needsApiKeyRef.current = needsApiKey;

  useEffect(() => {
    if (isOpen) {
      setPrompt(defaultPrompt);
      setModel(defaultModel ?? DEFAULT_SEEDREAM_MODEL);
      setSaveAsAutoEdit(false);
      setApiKey('');
      setShowApiKey(false);
      requestAnimationFrame(() => {
        if (needsApiKeyRef.current) {
          apiKeyInputRef.current?.focus();
        } else {
          textareaRef.current?.focus();
        }
      });
    }
  }, [isOpen, defaultPrompt, defaultModel]);

  const canConfirm = prompt.trim().length > 0 && (!needsApiKey || apiKey.trim().length > 0);

  const handleConfirm = async () => {
    if (!canConfirm) return;
    try {
      await onConfirm(prompt.trim(), model, saveAsAutoEdit, needsApiKey ? apiKey.trim() : undefined);
    } catch (err) {
      toast.error('AI 修图失败：' + formatError(err));
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && canConfirm) {
      e.preventDefault();
      handleConfirm();
    }
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onCancel}
      title="AI 修图"
      subtitle="使用生成式 AI 调整照片"
      icon={<div className="w-10 h-10 bg-gray-100 rounded-lg flex items-center justify-center"><Sparkles className="w-5 h-5 text-amber-600" /></div>}
      maxWidth="max-w-md"
      footer={
        <div className="flex items-center justify-between w-full">
          {autoEditEnabled && (
            <div className="flex items-center gap-2 cursor-pointer select-none" onClick={() => setSaveAsAutoEdit(!saveAsAutoEdit)}>
              <ToggleSwitch enabled={saveAsAutoEdit} onChange={setSaveAsAutoEdit} />
              <span className="text-sm font-medium text-gray-700">保存为自动修图设置</span>
            </div>
          )}
          {!autoEditEnabled && <div />}
          <div className="flex gap-2">
            <button
              onClick={onCancel}
              className="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 transition-colors text-sm"
            >
              取消
            </button>
            <button
              onClick={handleConfirm}
              disabled={!canConfirm}
              className="px-4 py-2 text-white bg-blue-600 rounded-lg hover:bg-blue-700 transition-colors text-sm disabled:opacity-50 disabled:cursor-not-allowed"
            >
              确认
            </button>
          </div>
        </div>
      }
    >
      <div className="space-y-3">
        {needsApiKey && (
          <ApiKeyField
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            show={showApiKey}
            onToggleShow={() => setShowApiKey(!showApiKey)}
            autoFocusRef={apiKeyInputRef}
          />
        )}
        <div className="space-y-2">
          <label className="block text-sm font-medium text-gray-700">模型</label>
          <Select
            value={model}
            options={SEEDREAM_MODELS}
            onChange={setModel}
          />
        </div>
        <div className="space-y-2">
          <label className="block text-sm font-medium text-gray-700">提示词</label>
          <textarea
            ref={textareaRef}
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="请输入提示词"
            rows={4}
            className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm bg-white text-gray-700 resize-none focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          />
        </div>
      </div>
    </Dialog>
  );
}
