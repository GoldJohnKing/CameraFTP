/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, useEffect, useRef } from 'react';
import { Sparkles } from 'lucide-react';
import { Dialog } from './ui/Dialog';
import { ToggleSwitch } from './ui/ToggleSwitch';
import { Select } from './ui/Select';
import { SEEDREAM_MODELS, DEFAULT_SEEDREAM_MODEL } from '../constants/seedream-models';

interface PromptDialogProps {
  isOpen: boolean;
  defaultPrompt: string;
  defaultModel?: string;
  autoEditEnabled?: boolean;
  onConfirm: (prompt: string, model: string, saveAsAutoEdit: boolean) => void;
  onCancel: () => void;
}

export function PromptDialog({ isOpen, defaultPrompt, defaultModel, autoEditEnabled, onConfirm, onCancel }: PromptDialogProps) {
  const [prompt, setPrompt] = useState(defaultPrompt);
  const [model, setModel] = useState(defaultModel ?? DEFAULT_SEEDREAM_MODEL);
  const [saveAsAutoEdit, setSaveAsAutoEdit] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (isOpen) {
      setPrompt(defaultPrompt);
      setModel(defaultModel ?? DEFAULT_SEEDREAM_MODEL);
      setSaveAsAutoEdit(false);
      requestAnimationFrame(() => textareaRef.current?.focus());
    }
  }, [isOpen, defaultPrompt, defaultModel]);

  const canConfirm = prompt.trim().length > 0;

  const handleConfirm = () => {
    if (!canConfirm) return;
    onConfirm(prompt.trim(), model, saveAsAutoEdit);
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
        <div className="space-y-1">
          <label className="block text-xs font-medium text-gray-500">模型</label>
          <Select
            value={model}
            options={SEEDREAM_MODELS}
            onChange={setModel}
          />
        </div>
        <div className="space-y-1">
          <label className="block text-xs font-medium text-gray-500">提示词</label>
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
