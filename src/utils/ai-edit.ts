/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { SEEDREAM_MODELS } from '../types';
import type { AppConfig } from '../types';

export interface AiEditCallContext {
  /**
   * Default prompt for UI dialogs (native Android dialog and web dialog).
   * Product ruling (#4): dialogs fall back to manualPrompt ONLY — when it is
   * empty the default stays empty; never fall back to the auto-edit prompt.
   */
  dialogPrompt: string;
  /**
   * Prompt for automatic (no-UI) AI editing triggered on file upload.
   * Product ruling (#4): this is aiEdit.prompt ONLY — it never falls back to
   * manualPrompt and must never be used as a dialog default.
   */
  autoPrompt: string;
  model: string;
  autoEdit: boolean;
  hasApiKey: boolean;
  /**
   * Available seedream model list for the native AI edit dialog dropdown.
   * Single source of truth: Rust SEEDREAM_MODELS (src-tauri/src/ai_edit/config.rs),
   * exported to TypeScript by gen-types (src-tauri/bindings/SeedreamModels.ts).
   */
  models: Array<{ value: string; label: string }>;
}

/**
 * Derive the AI edit call context from the current config draft.
 *
 * Prompt semantics (product ruling #4 — the two consumers never mix):
 * - dialogPrompt: manualPrompt only (may be empty) — used by all UI dialogs
 * - autoPrompt: aiEdit.prompt only — used exclusively by automatic AI editing
 *   (Rust-side on_file_uploaded reads aiEdit.prompt directly; it never sees
 *   manualPrompt)
 *
 * Model fallback chain (unchanged, outside ruling scope):
 * - model: manualModel → provider model (seed-edit provider only)
 * - hasApiKey: only meaningful for the seed-edit provider
 */
export function getAiEditCallContext(draft: AppConfig | null | undefined): AiEditCallContext {
  const manualModel = draft?.aiEdit?.manualModel || '';
  return {
    dialogPrompt: draft?.aiEdit?.manualPrompt || '',
    autoPrompt: draft?.aiEdit?.prompt || '',
    model: manualModel || (draft?.aiEdit?.provider?.type === 'seed-edit' ? draft.aiEdit.provider.model : '') || '',
    autoEdit: draft?.aiEdit?.autoEdit ?? false,
    hasApiKey: draft?.aiEdit?.provider?.type === 'seed-edit' ? !!draft.aiEdit.provider.apiKey : true,
    models: SEEDREAM_MODELS.map((m) => ({ value: m.value, label: m.label })),
  };
}
