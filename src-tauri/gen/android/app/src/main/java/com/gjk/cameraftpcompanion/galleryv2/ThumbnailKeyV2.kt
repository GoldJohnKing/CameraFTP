/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.galleryv2

import java.security.MessageDigest

/**
 * Deterministic cache key generator for V2 thumbnails.
 *
 * Keys are derived from media metadata so that any change to the source
 * (edit, rotation, re-encode) produces a different key and invalidates
 * the stale cached thumbnail.
 */
object ThumbnailKeyV2 {

    /**
     * Generate a deterministic cache key from media metadata.
     *
     * @param mediaId       Unique media identifier
     * @param dateModifiedMs Last modification timestamp in milliseconds
     * @param sizeBucket    Dimension bucket string (e.g. "256x256")
     * @param orientation   EXIF orientation value (0-8)
     * @param byteSize      File size in bytes
     * @return Lowercase hex SHA-1 digest (40 characters)
     */
    fun of(
        mediaId: String,
        dateModifiedMs: Long,
        sizeBucket: String,
        orientation: Int,
        byteSize: Long
    ): String {
        val input = "$mediaId:$dateModifiedMs:$sizeBucket:$orientation:$byteSize"
        val md = MessageDigest.getInstance("SHA-1")
        val digest = md.digest(input.toByteArray())
        return digest.joinToString("") { "%02x".format(it) }
    }
}
