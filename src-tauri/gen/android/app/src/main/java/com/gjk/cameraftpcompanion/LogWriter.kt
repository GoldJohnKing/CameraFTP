package com.gjk.cameraftpcompanion

/**
 * LogWriter stub - logging to external storage has been removed.
 * This object exists only to prevent breaking existing call sites.
 * All methods are no-ops.
 */
object LogWriter {
    fun init() {}
    fun log(message: String) {}
    fun logError(message: String, throwable: Throwable? = null) {}
    fun clear() {}
    fun getLogFilePath(): String? = null
}
