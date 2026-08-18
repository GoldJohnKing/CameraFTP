/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.app.Activity
import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.database.MatrixCursor
import android.net.Uri
import android.provider.MediaStore
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.shadows.ShadowContentResolver
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

/**
 * Exercises [GalleryActionsBridge.deleteImages] result classification through a
 * fake MediaStore-style provider registered with ShadowContentResolver, so
 * each test controls both the `rowsDeleted` return value and whether the
 * URI still exists afterwards (the two inputs of `classifyDeleteResult`).
 *
 * Limitation: the *confirmed* branch of the SecurityException flow
 * (`MediaStore.createDeleteRequest` → the system dialog → the blocking
 * latch in `MainActivity.requestDeleteConfirmation`) requires a real
 * MainActivity and system UI and is not drivable under Robolectric. These
 * tests cover the classification itself plus the routing decision:
 * SecurityException → pending confirmation (never directly `failed`),
 * with the un-confirmable request resolving to `deleteConfirmed = false`.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], manifest = Config.NONE)
class GalleryActionsBridgeDeleteTest {

    /**
     * ContentProvider double for a MediaStore-like collection. Behavior is
     * keyed by the numeric image id in the URI path so a single batch can
     * mix outcomes.
     */
    private class FakeDeleteProvider : ContentProvider() {
        var defaultDeleteResult: Int = 1
        var defaultQueryRowCount: Int = 0

        /** image id → rowsDeleted return value. */
        val deleteResults = mutableMapOf<Int, Int>()

        /** image id → row count returned by classification queries. */
        val queryRowCounts = mutableMapOf<Int, Int>()

        /** image id → exception thrown by delete (e.g. SecurityException). */
        val deleteErrors = mutableMapOf<Int, Exception>()

        private fun idOf(uri: Uri): Int = uri.pathSegments.lastOrNull()?.toIntOrNull() ?: 0

        override fun onCreate(): Boolean = true

        override fun query(
            uri: Uri,
            projection: Array<out String>?,
            selection: String?,
            selectionArgs: Array<out String>?,
            sortOrder: String?
        ): Cursor? = MatrixCursor(arrayOf(MediaStore.Images.Media._ID)).apply {
            repeat(queryRowCounts[idOf(uri)] ?: defaultQueryRowCount) { row ->
                addRow(arrayOf<Any>(row.toLong()))
            }
        }

        override fun getType(uri: Uri): String? = "image/jpeg"

        override fun insert(uri: Uri, values: ContentValues?): Uri? = null

        override fun delete(
            uri: Uri,
            selection: String?,
            selectionArgs: Array<out String>?
        ): Int {
            deleteErrors[idOf(uri)]?.let { throw it }
            return deleteResults[idOf(uri)] ?: defaultDeleteResult
        }

        override fun update(
            uri: Uri,
            values: ContentValues?,
            selection: String?,
            selectionArgs: Array<out String>?
        ): Int = 0
    }

    private lateinit var activity: Activity
    private lateinit var bridge: GalleryActionsBridge
    private lateinit var provider: FakeDeleteProvider

    @Before
    fun setUp() {
        activity = Robolectric.buildActivity(Activity::class.java).create().get()
        bridge = GalleryActionsBridge(activity)
        provider = FakeDeleteProvider()
        // Registered under this test's own authority (not "media") so the
        // registration neither depends on nor disturbs any global setup.
        ShadowContentResolver.registerProviderInternal("gjk-delete-test", provider)
    }

    private fun uri(id: Int) = "content://gjk-delete-test/images/$id"

    private fun deleteImages(vararg uris: String): JSONObject =
        JSONObject(bridge.deleteImages(JSONArray(uris.toList()).toString()))

    private fun JSONArray.strings(): List<String> =
        (0 until length()).map { getString(it) }

    // ── Classification by rowsDeleted / still-exists ───────────────────

    @Test
    fun delete_with_rows_deleted_and_gone_classifies_deleted() {
        val result = deleteImages(uri(1))

        assertEquals(listOf(uri(1)), result.getJSONArray("deleted").strings())
        assertEquals(emptyList<String>(), result.getJSONArray("notFound").strings())
        assertEquals(emptyList<String>(), result.getJSONArray("failed").strings())
    }

    @Test
    fun delete_with_zero_rows_and_gone_classifies_not_found() {
        provider.defaultDeleteResult = 0

        val result = deleteImages(uri(2))

        assertEquals(listOf(uri(2)), result.getJSONArray("notFound").strings())
        assertEquals(emptyList<String>(), result.getJSONArray("deleted").strings())
        assertEquals(emptyList<String>(), result.getJSONArray("failed").strings())
    }

    @Test
    fun delete_with_rows_deleted_but_still_existing_classifies_failed() {
        // rowsDeleted > 0 but the URI still resolves — the row survived.
        provider.defaultDeleteResult = 1
        provider.defaultQueryRowCount = 1

        val result = deleteImages(uri(3))

        assertEquals(listOf(uri(3)), result.getJSONArray("failed").strings())
        assertEquals(emptyList<String>(), result.getJSONArray("deleted").strings())
        assertEquals(emptyList<String>(), result.getJSONArray("notFound").strings())
    }

    @Test
    fun delete_mixed_batch_classifies_each_uri_independently() {
        provider.deleteResults[1] = 1   // deleted
        provider.deleteResults[2] = 0   // notFound
        provider.deleteResults[3] = 1   // failed (still exists)
        provider.queryRowCounts[3] = 1

        val result = deleteImages(uri(1), uri(2), uri(3))

        assertEquals(listOf(uri(1)), result.getJSONArray("deleted").strings())
        assertEquals(listOf(uri(2)), result.getJSONArray("notFound").strings())
        assertEquals(listOf(uri(3)), result.getJSONArray("failed").strings())
    }

    // ── Exception routing ──────────────────────────────────────────────

    @Test
    fun delete_non_security_exception_classifies_failed() {
        provider.deleteErrors[4] = IllegalArgumentException("simulated malformed uri")

        val result = deleteImages(uri(4))

        assertEquals(listOf(uri(4)), result.getJSONArray("failed").strings())
        assertEquals(emptyList<String>(), result.getJSONArray("deleted").strings())
        assertEquals(emptyList<String>(), result.getJSONArray("notFound").strings())
    }

    @Test
    fun delete_security_exception_routes_to_confirmation_not_failed() {
        provider.deleteErrors[5] = SecurityException("needs user confirmation")

        val result = deleteImages(uri(5))

        // SecurityException never lands in `failed` directly — it goes to
        // pending confirmation. Under Robolectric the confirmation request
        // cannot be approved (no MainActivity system dialog), so it resolves
        // to deleteConfirmed=false → rows=0 → URI gone → notFound.
        assertEquals(
            "SecurityException must not be classified as failed",
            emptyList<String>(),
            result.getJSONArray("failed").strings()
        )
        assertEquals(listOf(uri(5)), result.getJSONArray("notFound").strings())
    }

    @Test
    fun delete_security_exception_still_existing_after_refusal_classifies_failed() {
        provider.deleteErrors[6] = SecurityException("needs user confirmation")
        provider.queryRowCounts[6] = 1

        val result = deleteImages(uri(6))

        assertEquals(
            "Refused confirmation with the row still present must classify failed",
            listOf(uri(6)),
            result.getJSONArray("failed").strings()
        )
    }

    // ── Edge cases ─────────────────────────────────────────────────────

    @Test
    fun delete_empty_uri_list_returns_empty_result() {
        val result = deleteImages()

        assertEquals(0, result.getJSONArray("deleted").length())
        assertEquals(0, result.getJSONArray("notFound").length())
        assertEquals(0, result.getJSONArray("failed").length())
    }

    @Test
    fun delete_invalid_json_returns_empty_result() {
        val result = JSONObject(bridge.deleteImages("not json"))

        assertEquals(0, result.getJSONArray("deleted").length())
        assertEquals(0, result.getJSONArray("notFound").length())
        assertEquals(0, result.getJSONArray("failed").length())
    }

    @Test
    fun delete_attempts_one_resolver_delete_per_uri() {
        deleteImages(uri(1), uri(2), uri(3))

        assertEquals(
            "Every requested URI must produce exactly one delete attempt",
            listOf(uri(1), uri(2), uri(3)),
            shadowOf(activity.contentResolver).deleteStatements.map { it.uri.toString() }
        )
    }
}
