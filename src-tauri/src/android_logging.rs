// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Android logging: logcat is the only sink.

use std::io::Write;
use tracing_subscriber::fmt::MakeWriter;

extern "C" {
    fn __android_log_write(prio: i32, tag: *const u8, text: *const u8) -> i32;
}

const TAG: &[u8] = b"CameraFTP\0";
// android/log.h severity constants.
const LOG_ERROR: i32 = 6;
const LOG_WARN: i32 = 5;
const LOG_INFO: i32 = 4;
const LOG_DEBUG: i32 = 3;

/// tracing fmt writer that line-buffers into logcat. A fresh buffer per event
/// (MakeWriter hands one out per event), so events don't interleave at the
/// buffer level; the trailing partial line is flushed on Drop.
struct AndroidLogWriter {
    prio: i32,
    buf: Vec<u8>,
}

impl AndroidLogWriter {
    fn new(prio: i32) -> Self {
        Self { prio, buf: Vec::new() }
    }
    fn emit_line(&mut self) {
        if self.buf.is_empty() {
            return;
        }
        self.buf.push(0); // NUL-terminate for the C string
        // SAFETY: TAG and self.buf are both NUL-terminated; prio is a constant.
        unsafe {
            __android_log_write(self.prio, TAG.as_ptr(), self.buf.as_ptr());
        }
        self.buf.clear();
    }
}

impl Write for AndroidLogWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        for &b in bytes {
            if b == b'\n' {
                self.emit_line();
            } else {
                self.buf.push(b);
            }
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for AndroidLogWriter {
    fn drop(&mut self) {
        self.emit_line();
    }
}

struct AndroidLogMakeWriter;
impl<'a> MakeWriter<'a> for AndroidLogMakeWriter {
    type Writer = AndroidLogWriter;
    fn make_writer(&'a self) -> Self::Writer {
        AndroidLogWriter::new(LOG_INFO)
    }
    fn make_writer_for(&'a self, meta: &tracing::Metadata<'_>) -> Self::Writer {
        let prio = match *meta.level() {
            tracing::Level::ERROR => LOG_ERROR,
            tracing::Level::WARN => LOG_WARN,
            tracing::Level::INFO => LOG_INFO,
            tracing::Level::DEBUG | tracing::Level::TRACE => LOG_DEBUG,
        };
        AndroidLogWriter::new(prio)
    }
}

/// Initialize Android logging: logcat only.
pub fn setup() {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    #[cfg(debug_assertions)]
    let env_filter = EnvFilter::new("debug");
    #[cfg(not(debug_assertions))]
    let env_filter = EnvFilter::new("info");

    let logcat_layer = tracing_subscriber::fmt::layer()
        .with_writer(AndroidLogMakeWriter)
        .with_ansi(false)
        .with_target(true);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(logcat_layer)
        .init();
}
