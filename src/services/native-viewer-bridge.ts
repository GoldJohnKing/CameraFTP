/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { applyAndEnqueueAiEdit, getCurrentAiEditProgress } from './ai-edit-tasks';
import { getCurrentColorGradingProgress } from './color-grading-tasks';
import { getCachedColorGradingPresets } from '../hooks/useColorGradingPresets';
import { useConfigStore } from '../stores/configStore';
import { usePermissionStore } from '../stores/permissionStore';
import { applyColorGradingLastUsed } from '../utils/color-grading';
import { getAiEditCallContext } from '../utils/ai-edit';
import { PERMISSION_RECHECK_REQUESTED_EVENT } from '../utils/gallery-refresh';
import { requestExifForPositions } from './image-open';

/**
 * Register the window.__tauri* JS bridges consumed by the native Android
 * ImageViewerActivity / ColorGradingActivity (AI edit, color grading,
 * previews), plus the global EXIF prefetch handler
 * (window.__requestExifForPositions) called via evaluateJavascript.
 *
 * Returns a cleanup function that deletes every registration.
 */
export function registerNativeViewerBridges(): () => void {
  const w = window as unknown as Record<string, unknown>;

  // MainActivity dispatches this from onResume (e.g. after returning from
  // system permission settings). Re-check Android permissions; the storage
  // false→true transition hook inside setPermissions then refreshes the
  // gallery.
  const handlePermissionRecheckRequest = () => {
    void usePermissionStore.getState().checkPermissions();
  };
  window.addEventListener(PERMISSION_RECHECK_REQUESTED_EVENT, handlePermissionRecheckRequest);

  w.__tauriGetAiEditPrompt = () => {
    return JSON.stringify(getAiEditCallContext(useConfigStore.getState().draft));
  };

  w.__tauriTriggerAiEditWithPrompt = async (filePath: string, prompt: string, model?: string, saveAsAutoEdit?: boolean, apiKey?: string) => {
    await applyAndEnqueueAiEdit({ filePaths: [filePath], prompt, model: model ?? '', saveAsAutoEdit, apiKey });
  };

  w.__tauriGetAiEditProgress = () => {
    return getCurrentAiEditProgress();
  };

  w.__tauriCancelAiEdit = async () => {
    const { cancelAiEdit } = await import('./ai-edit-tasks');
    await cancelAiEdit();
  };

  w.__tauriGetAutoColorGradingEnabled = () => {
    const draft = useConfigStore.getState().draft;
    return String(draft?.autoColorGrading?.enabled ?? false);
  };

  w.__tauriGetColorGradingLastUsed = () => {
    const draft = useConfigStore.getState().draft;
    return JSON.stringify(draft?.colorGradingLastUsed ?? null);
  };

  w.__tauriGetColorGradingPresets = () => {
    return JSON.stringify(getCachedColorGradingPresets().map(p => [p.id, p.displayName]));
  };

  w.__tauriTriggerColorGrading = async (filePath: string, lutId: string, meteringMode: string, evOffset: number, syncToAuto: boolean) => {
    const { enqueueColorGrading } = await import('./color-grading-tasks');
    await enqueueColorGrading([filePath], lutId, meteringMode, evOffset);

    useConfigStore.getState().updateDraft(d => applyColorGradingLastUsed(
      d,
      { presetId: lutId, meteringMode, evOffset },
      syncToAuto,
    ));
  };

  w.__tauriGetColorGradingProgress = () => {
    return getCurrentColorGradingProgress();
  };

  w.__tauriCancelColorGrading = async () => {
    const { cancelColorGrading } = await import('./color-grading-tasks');
    await cancelColorGrading();
  };

  w.__tauriSaveColorGradingLastUsed = (lutId: string, meteringMode: string, evOffset: number) => {
    useConfigStore.getState().updateDraft(d => applyColorGradingLastUsed(
      d,
      { presetId: lutId, meteringMode, evOffset },
      false,
    ));
  };

  // Global handler for EXIF prefetch requests from native viewer.
  // Called via evaluateJavascript from ImageViewerActivity.
  w.__requestExifForPositions = (requestJson: string) => {
    try {
      const items = JSON.parse(requestJson) as Array<{
        position: number;
        uri: string;
      }>;
      void requestExifForPositions(items);
    } catch {
      // Ignore malformed requests
    }
  };

  return () => {
    window.removeEventListener(PERMISSION_RECHECK_REQUESTED_EVENT, handlePermissionRecheckRequest);
    delete w.__tauriGetAiEditPrompt;
    delete w.__tauriTriggerAiEditWithPrompt;
    delete w.__tauriGetAiEditProgress;
    delete w.__tauriCancelAiEdit;
    delete w.__tauriGetAutoColorGradingEnabled;
    delete w.__tauriGetColorGradingLastUsed;
    delete w.__tauriGetColorGradingPresets;
    delete w.__tauriTriggerColorGrading;
    delete w.__tauriGetColorGradingProgress;
    delete w.__tauriCancelColorGrading;
    delete w.__tauriSaveColorGradingLastUsed;
    delete w.__requestExifForPositions;
  };
}
