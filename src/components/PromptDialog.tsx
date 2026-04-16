/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, useEffect, useRef } from 'react';
import { Dialog } from './ui/Dialog';
import { ToggleSwitch } from './ui/ToggleSwitch';

interface PromptDialogProps {
  isOpen: boolean;
  defaultPrompt: string;
  onConfirm: (prompt: string, shouldSave: boolean) => void;
  onCancel: () => void;
}

export function PromptDialog({ isOpen, defaultPrompt, onConfirm, onCancel }: PromptDialogProps) {
  const [prompt, setPrompt] = useState(defaultPrompt);
  const [savePrompt, setSavePrompt] = useState(true);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (isOpen) {
      setPrompt(defaultPrompt);
      setSavePrompt(true);
      // Focus textarea after dialog opens
      requestAnimationFrame(() => textareaRef.current?.focus());
    }
  }, [isOpen, defaultPrompt]);

  const handleConfirm = () => {
    const trimmed = prompt.trim();
    onConfirm(trimmed, savePrompt);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      handleConfirm();
    }
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onCancel}
      title="AI修图提示词"
      subtitle="编辑提示词后确认触发修图"
      maxWidth="max-w-lg"
      footer={
        <div className="flex items-center justify-between w-full">
          <ToggleSwitch
            enabled={savePrompt}
            onChange={setSavePrompt}
            label="保存提示词"
          />
          <div className="flex gap-2">
            <button
              onClick={onCancel}
              className="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 transition-colors text-sm"
            >
              取消
            </button>
            <button
              onClick={handleConfirm}
              className="px-4 py-2 text-white bg-blue-600 rounded-lg hover:bg-blue-700 transition-colors text-sm"
            >
              确认修图
            </button>
          </div>
        </div>
      }
    >
      <div className="space-y-3">
        <textarea
          ref={textareaRef}
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="例如：提升画质，使照片更清晰"
          rows={4}
          className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm bg-white text-gray-700 resize-none focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
        />
        <p className="text-xs text-gray-400">
          留空使用默认提示词 · Ctrl+Enter 快速确认
        </p>
      </div>
    </Dialog>
  );
}
