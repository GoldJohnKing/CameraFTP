/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { Eye, EyeOff, ExternalLink } from 'lucide-react';
import { MaskedInput } from './MaskedInput';
import { openExternalLink } from '../../utils/external-link';

/**
 * Default input styling (focus-ring variant, always enabled).
 * Call sites that disable the input pass their own `inputClassName`
 * (disabled variant, no focus ring) to stay pixel-identical.
 */
const DEFAULT_INPUT_CLASS_NAME =
  'w-full px-3 py-2 pr-10 border border-gray-200 rounded-lg text-sm bg-white text-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent';

const TOGGLE_BUTTON_CLASS_NAME =
  'absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 disabled:opacity-50';

const LINK_BUTTON_CLASS_NAME =
  'text-sm text-blue-600 hover:text-blue-700 inline-flex items-center gap-0.5 mt-1';

export interface ApiKeyFieldProps {
  /** Current (unmasked) API key value. */
  value: string;
  /** Change handler receiving the real (unmasked) value via e.target.value. */
  onChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  /** Optional blur handler (commits the value). */
  onBlur?: () => void;
  /** Disables both the input and the eye toggle button. */
  disabled?: boolean;
  /** Whether the real value is shown (vs masked dots). */
  show: boolean;
  /** Toggles visibility. */
  onToggleShow: () => void;
  /** Field label. */
  label?: string;
  /** Ref forwarded to the masked input (e.g. for autofocus). */
  autoFocusRef?: React.Ref<HTMLInputElement>;
  /** Optional override for the masked input className. */
  inputClassName?: string;
}

/**
 * Volcengine (火山引擎) API key input field: masked input + eye toggle
 * button + "开通火山引擎模型服务" external link.
 */
export function ApiKeyField({
  value,
  onChange,
  onBlur,
  disabled,
  show,
  onToggleShow,
  label = '火山引擎 API Key',
  autoFocusRef,
  inputClassName = DEFAULT_INPUT_CLASS_NAME,
}: ApiKeyFieldProps) {
  return (
    <div className="space-y-2">
      <label className="block text-sm font-medium text-gray-700">{label}</label>
      <div className="relative">
        <MaskedInput
          ref={autoFocusRef}
          visible={show}
          value={value}
          onChange={onChange}
          onBlur={onBlur}
          placeholder="输入火山引擎 API Key"
          disabled={disabled}
          className={inputClassName}
        />
        <button
          type="button"
          onMouseDown={(e) => e.preventDefault()}
          onClick={onToggleShow}
          disabled={disabled}
          className={TOGGLE_BUTTON_CLASS_NAME}
        >
          {show ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
        </button>
      </div>
      <button
        onClick={() => openExternalLink('https://www.volcengine.com/docs/82379/1399008')}
        className={LINK_BUTTON_CLASS_NAME}
        type="button"
      >
        开通火山引擎模型服务
        <ExternalLink className="w-3 h-3" />
      </button>
    </div>
  );
}
