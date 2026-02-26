use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use windows::Win32::System::Com::CoInitialize;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::error::AppError;

/// 使用系统默认程序打开
pub fn open_with_default(file_path: &PathBuf) -> Result<(), AppError> {
    open_with_shell_execute(file_path, None)
}

/// 使用 Windows 照片应用打开
pub fn open_with_photos(file_path: &PathBuf) -> Result<(), AppError> {
    // 尝试使用 ms-photos:viewer?fileName= URI scheme
    let path_str = file_path.to_string_lossy();
    let uri = format!(
        "ms-photos:viewer?fileName={}",
        urlencoding::encode(&path_str)
    );

    // 将 URI 转换为 PathBuf 以便复用函数
    let uri_path = PathBuf::from(uri);
    open_with_shell_execute(&uri_path, None)
        .or_else(|_| open_with_shell_execute(file_path, Some("open")))
}

/// 使用自定义程序打开
pub fn open_with_program(file_path: &PathBuf, program: &str) -> Result<(), AppError> {
    // 对于自定义程序，我们需要使用 program 作为操作，file_path 作为参数
    // 这里我们使用 runas 操作来执行自定义程序
    open_with_program_execute(file_path, program)
}

fn open_with_shell_execute(file_path: &PathBuf, operation: Option<&str>) -> Result<(), AppError> {
    unsafe {
        let _ = CoInitialize(None);
    }

    // 直接将 PathBuf 转换为宽字符
    let file_wide: Vec<u16> = file_path.as_os_str().encode_wide().chain(Some(0)).collect();

    // 处理 operation 参数
    let operation_ptr = if let Some(op) = operation {
        let op_wide: Vec<u16> = OsStr::new(op).encode_wide().chain(Some(0)).collect();
        windows::core::PCWSTR(op_wide.as_ptr())
    } else {
        windows::core::PCWSTR(std::ptr::null())
    };

    let result = unsafe {
        ShellExecuteW(
            None,
            operation_ptr,
            windows::core::PCWSTR(file_wide.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };

    // ShellExecuteW 返回 HINSTANCE，成功时大于 32，失败时小于等于 32
    // 在 windows 0.58 中 HINSTANCE 是一个结构体，其 .0 字段是 *mut c_void
    if result.0 as usize <= 32 {
        return Err(AppError::Other(format!(
            "ShellExecute failed with code {:?}",
            result.0
        )));
    }

    Ok(())
}

fn open_with_program_execute(file_path: &PathBuf, program: &str) -> Result<(), AppError> {
    unsafe {
        let _ = CoInitialize(None);
    }

    // 程序路径转换为宽字符
    let program_wide: Vec<u16> = OsStr::new(program).encode_wide().chain(Some(0)).collect();

    // 文件路径转换为宽字符
    let file_wide: Vec<u16> = file_path.as_os_str().encode_wide().chain(Some(0)).collect();

    let result = unsafe {
        ShellExecuteW(
            None,
            None,
            windows::core::PCWSTR(program_wide.as_ptr()),
            windows::core::PCWSTR(file_wide.as_ptr()),
            None,
            SW_SHOWNORMAL,
        )
    };

    if result.0 as usize <= 32 {
        return Err(AppError::Other(format!(
            "ShellExecute failed with code {:?}",
            result.0
        )));
    }

    Ok(())
}
