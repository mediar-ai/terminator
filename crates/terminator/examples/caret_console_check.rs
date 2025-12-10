//! Check caret position via Console APIs (for cmd/PowerShell)

fn main() {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::System::Console::{
            GetConsoleScreenBufferInfo, GetStdHandle, CONSOLE_SCREEN_BUFFER_INFO,
            STD_OUTPUT_HANDLE,
        };

        let handle = GetStdHandle(STD_OUTPUT_HANDLE).unwrap();
        let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();

        if GetConsoleScreenBufferInfo(handle, &mut info).is_ok() {
            println!("Console cursor position: ({}, {})",
                info.dwCursorPosition.X,
                info.dwCursorPosition.Y);
            println!("Window size: {}x{}",
                info.srWindow.Right - info.srWindow.Left + 1,
                info.srWindow.Bottom - info.srWindow.Top + 1);
            println!("Buffer size: {}x{}",
                info.dwSize.X,
                info.dwSize.Y);
        } else {
            println!("GetConsoleScreenBufferInfo failed (not a console window?)");
        }
    }
}
