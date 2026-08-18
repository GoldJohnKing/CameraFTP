/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { ApiKeyField } from './ui/ApiKeyField';
import { Select } from './ui/Select';
import { SEEDREAM_MODELS } from '../types';

/** Props forwarded to the optional 火山引擎 API key field. */
export interface AiEditApiKeyFieldProps {
  /** Current (unmasked) API key value. */
  value: string;
  /** Change handler receiving the real (unmasked) value via e.target.value. */
  onChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  /** Whether the real value is shown (vs masked dots). */
  show: boolean;
  /** Toggles visibility. */
  onToggleShow: () => void;
  /** Optional blur handler (commits the value). */
  onBlur?: () => void;
  /** Disables both the input and the eye toggle button. */
  disabled?: boolean;
  /** Ref forwarded to the masked input (e.g. for autofocus). */
  autoFocusRef?: React.Ref<HTMLInputElement>;
  /** Optional override for the masked input className. */
  inputClassName?: string;
}

/** Props for the optional Seedream 模型 select field. */
export interface AiEditModelFieldProps {
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}

/** Props for the 提示词 textarea field. */
export interface AiEditPromptFieldProps {
  value: string;
  onChange: (e: React.ChangeEvent<HTMLTextAreaElement>) => void;
  /** Row count (4 in the dialog, 1 with auto-resize in the config panel). */
  rows: number;
  /** Exact textarea className (focus-ring dialog variant vs disabled panel variant). */
  className: string;
  /** Ref forwarded to the textarea (RefObject or auto-resize callback). */
  textareaRef?: React.Ref<HTMLTextAreaElement>;
  /** Optional keydown handler (e.g. Ctrl+Enter to confirm). */
  onKeyDown?: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void;
  /** Optional blur handler (commits the value). */
  onBlur?: () => void;
  disabled?: boolean;
  /** When set, rendered as a red hint below the textarea. */
  error?: string;
}

interface AiEditFormFieldsProps {
  /** Pass null to omit the API key field (e.g. when a key is already stored). */
  apiKey?: AiEditApiKeyFieldProps | null;
  /** Pass null to omit the 模型 field (e.g. when auto-edit is disabled). */
  model?: AiEditModelFieldProps | null;
  /** Pass null to omit the 提示词 field (e.g. when auto-edit is disabled). */
  prompt?: AiEditPromptFieldProps | null;
  /** Rendered between the API key field and the 模型 field (e.g. the auto-edit toggle). */
  children?: React.ReactNode;
}

/**
 * Presentational field set shared by the AI-edit dialog and the AI-edit
 * config panel: 火山引擎 API key + Seedream 模型 select + 提示词 textarea.
 * Owns no state; every field is controlled via props.
 */
export function AiEditFormFields({ apiKey, model, prompt, children }: AiEditFormFieldsProps) {
  return (
    <>
      {apiKey && (
        <ApiKeyField
          value={apiKey.value}
          onChange={apiKey.onChange}
          onBlur={apiKey.onBlur}
          disabled={apiKey.disabled}
          show={apiKey.show}
          onToggleShow={apiKey.onToggleShow}
          autoFocusRef={apiKey.autoFocusRef}
          inputClassName={apiKey.inputClassName}
        />
      )}
      {children}
      {model && (
        <div className="space-y-2">
          <label className="block text-sm font-medium text-gray-700">模型</label>
          <Select
            value={model.value}
            options={SEEDREAM_MODELS}
            onChange={model.onChange}
            disabled={model.disabled}
          />
        </div>
      )}
      {prompt && (
        <div className="space-y-2">
          <label className="block text-sm font-medium text-gray-700">提示词</label>
          <textarea
            ref={prompt.textareaRef}
            value={prompt.value}
            onChange={prompt.onChange}
            onKeyDown={prompt.onKeyDown}
            onBlur={prompt.onBlur}
            placeholder="请输入提示词"
            rows={prompt.rows}
            disabled={prompt.disabled}
            className={prompt.className}
          />
          {prompt.error && (
            <p className="text-xs text-red-500">{prompt.error}</p>
          )}
        </div>
      )}
    </>
  );
}
