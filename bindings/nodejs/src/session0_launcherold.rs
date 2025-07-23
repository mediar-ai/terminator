use windows::{
    Win32::System::Threading::{
        CreateProcessAsUserW, PROCESS_INFORMATION, STARTUPINFOW,
    },
    Win32::Security::Authentication::Identity::WTSQueryUserToken,
    Win32::System::RemoteDesktop::WTSGetActiveConsoleSessionId,
    Win32::Foundation::{CloseHandle, HANDLE},
};

use std::ptr::null_mut;

pub fn launch_in_session0(app_name: &str) {
    unsafe {
        let session_id = WTSGetActiveConsoleSessionId();

        let mut user_token: HANDLE = HANDLE::default();
        let result = WTSQueryUserToken(session_id, &mut user_token);

        if !result.as_bool() {
            println!("❌ Failed to get user token.");
            return;
        }

        let mut startup_info = STARTUPINFOW::default();
        let mut process_info = PROCESS_INFORMATION::default();

        let app: Vec<u16> = format!("{}\0", app_name).encode_utf16().collect();

        let created = CreateProcessAsUserW(
            user_token,
            None,
            &app,
            None,
            None,
            false,
            Default::default(),
            None,
            None,
            &mut startup_info,
            &mut process_info,
        );

        if created.as_bool() {
            println!("✅ App launched in Session 0.");
        } else {
            println!("❌ Failed to launch app in Session 0.");
        }

        CloseHandle(user_token);
    }
}
