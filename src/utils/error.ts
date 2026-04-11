/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

/**
 * Error handling utilities
 * Provides helpers for silent error handling and fallback values
 */

/**
 * Format an error into a human-readable string
 */
export function formatError(err: unknown): string {
    if (err instanceof Error) {
        return err.message;
    }
    if (typeof err === 'string') {
        return err;
    }
    if (err == null) {
        return 'Unknown error';
    }
    try {
        return JSON.stringify(err);
    } catch {
        return String(err);
    }
}

/**
 * Execute an async function and return null on error (silent fail)
 * Use when you don't care about errors and just want the result or nothing
 */
export async function silent<T>(fn: () => Promise<T>): Promise<T | null> {
    try {
        return await fn();
    } catch {
        return null;
    }
}
