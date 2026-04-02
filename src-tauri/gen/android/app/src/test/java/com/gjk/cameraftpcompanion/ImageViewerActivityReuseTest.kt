/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class ImageViewerActivityReuseTest {

    @Test
    fun build_navigation_target_when_uri_exists() {
        val uris = listOf("content://media/1", "content://media/2", "content://media/3")

        val target = ImageViewerActivity.buildNavigationTarget(uris, "content://media/2")

        assertNotNull(target)
        assertEquals(uris, target?.uris)
        assertEquals(1, target?.targetIndex)
    }

    @Test
    fun return_null_navigation_target_when_uri_missing() {
        val uris = listOf("content://media/1", "content://media/2")

        val target = ImageViewerActivity.buildNavigationTarget(uris, "content://media/9")

        assertNull(target)
    }

    @Test
    fun build_reuse_plan_for_existing_activity_clamps_target_index() {
        val uris = listOf("content://media/1", "content://media/2", "content://media/3")

        val plan = ImageViewerActivity.buildReuseNavigationPlan(
            hasVisibleReusableViewer = true,
            targetUris = uris,
            targetIndex = 999,
        )

        assertNotNull(plan)
        assertEquals(true, plan?.shouldReuseExisting)
        assertEquals(uris, plan?.uris)
        assertEquals(2, plan?.safeTargetIndex)
    }

    @Test
    fun build_reuse_plan_returns_null_when_target_uris_empty() {
        val plan = ImageViewerActivity.buildReuseNavigationPlan(
            hasVisibleReusableViewer = true,
            targetUris = emptyList(),
            targetIndex = 0,
        )

        assertNull(plan)
    }

    @Test
    fun build_reuse_plan_disables_reuse_when_viewer_not_visible() {
        val uris = listOf("content://media/1", "content://media/2")

        val plan = ImageViewerActivity.buildReuseNavigationPlan(
            hasVisibleReusableViewer = false,
            targetUris = uris,
            targetIndex = 1,
        )

        assertNotNull(plan)
        assertEquals(false, plan?.shouldReuseExisting)
        assertEquals(1, plan?.safeTargetIndex)
    }
}
