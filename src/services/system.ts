/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { invoke } from '@tauri-apps/api/core';

/** Query whether the app is registered to launch on system startup. */
export async function getAutostartStatus(): Promise<boolean> {
  return invoke<boolean>('get_autostart_status');
}

/** Open the native directory picker; resolves the chosen path, or null if cancelled. */
export async function selectSaveDirectory(): Promise<string | null> {
  return invoke<string | null>('select_save_directory');
}

/** Open the configured save directory in the system file manager. */
export async function openSaveDirectory(): Promise<void> {
  return invoke<void>('open_save_directory');
}

/** Open the containing folder of the given file with the file selected. */
export async function openFolderSelectFile(filePath: string): Promise<void> {
  return invoke<void>('open_folder_select_file', { filePath });
}

/** Open the native executable picker; resolves the chosen path, or null if cancelled. */
export async function selectExecutableFile(): Promise<string | null> {
  return invoke<string | null>('select_executable_file');
}
