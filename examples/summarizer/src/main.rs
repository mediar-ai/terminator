use arboard::Clipboard;
use std::{
    io::{self, Write},
    process::Command,
    thread,
    time::Duration,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, EnumWindows, IsWindowVisible,
};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};

fn get_foreground_window_title() -> String {
    unsafe {
        let hwnd = GetForegroundWindow();
        let len = GetWindowTextLengthW(hwnd) + 1;
        let mut buffer = vec![0u16; len as usize];
        let _ = GetWindowTextW(hwnd, &mut buffer);
        String::from_utf16_lossy(&buffer)
            .trim_matches(char::from(0))
            .trim()
            .to_string()
    }
}

fn get_ui_context() -> String {
    unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let titles = &mut *(lparam.0 as *mut Vec<String>);

        if IsWindowVisible(hwnd).as_bool() {
            let len = GetWindowTextLengthW(hwnd);
            if len > 0 {
                let mut buffer = vec![0u16; (len + 1) as usize];
                if GetWindowTextW(hwnd, &mut buffer) > 0 {
                    let title = String::from_utf16_lossy(&buffer)
                        .trim_matches(char::from(0))
                        .trim()
                        .to_string();
                    if !title.is_empty() {
                        titles.push(title);
                    }
                }
            }
        }

        BOOL(1) // continue enumeration (TRUE)
    }

    let mut titles: Vec<String> = Vec::new();
    unsafe {
        EnumWindows(Some(enum_windows_proc), LPARAM(&mut titles as *mut _ as isize));
    }

    titles.join("\n")
}

fn run_ollama(prompt: &str) -> String {
    let output = Command::new("ollama")
        .args(["run", "gemma3", prompt])
        .output()
        .expect("❌ Failed to run Ollama");

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn extract_numbered_points(summary: &str) -> Vec<String> {
    summary
        .lines()
        .filter(|line| {
            let line = line.trim();
            !line.is_empty()
                && !line.starts_with("---")
                && !line.starts_with("Okay")
                && !line.starts_with("Would you like")
        })
        .map(|line| line.trim().trim_matches('"').to_string())
        .collect()
}

fn main() {
    println!("⏳ Waiting for Ctrl+J to be pressed...");
    thread::sleep(Duration::from_secs(5));

    println!("⌨ Hotkey triggered. Capturing UI context...");
    let context = get_ui_context();

    let prompt = format!(
        r#"
You're helping build a local AI assistant that can understand what the user is doing based on their active screen context.

Here is a raw string extracted from the user's currently focused window titles:
"{context}"

Your job is to:
1. Analyze this data and identify **exactly what** the user might be doing — be specific (e.g., "solving Leetcode 2 Sum problem", "viewing John Smith's LinkedIn profile", "writing an email", "editing main.rs").
2. Infer which websites or tasks are active from window titles.
3. Group your response into:
   - **Summary**: One-line, high-confidence guess.
   - **Details**: A breakdown of the current windows and what they suggest.
   - **Possible Next Help**: What the assistant can help with now.

Respond with markdown, no follow-up questions or meta-comments.
"#
    );

    println!("🤖 Sending prompt to Ollama...");
    let summary = run_ollama(&prompt);

    let mut clipboard = Clipboard::new().unwrap();
    clipboard.set_text(summary.clone()).unwrap();

    println!("✅ Copied summary to clipboard:\n{}", summary);

    let points = extract_numbered_points(&summary);

    if !points.is_empty() {
        loop {
            println!("\n💡 Which part would you like me to refine?");
            for (i, point) in points.iter().enumerate() {
                println!("{}. {}", i + 1, point);
            }
            println!("0. Exit");

            print!("Enter your choice: ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();

            match input.trim() {
                "0" | "exit" => {
                    println!("👋 Exiting...");
                    break;
                }
                n if n.parse::<usize>().is_ok() => {
                    let index = n.parse::<usize>().unwrap();
                    if index >= 1 && index <= points.len() {
                        let sub_prompt = format!(
                            "Please elaborate on the following context in detail:\n\"{}\"",
                            points[index - 1]
                        );
                        println!("🔍 Refining...\n");
                        let refined = run_ollama(&sub_prompt);
                        println!("✨ Refined Output:\n{}", refined);
                    } else {
                        println!("❌ Invalid option.");
                    }
                }
                _ => println!("❌ Invalid input."),
            }
        }
    } else {
        println!("⚠️ No bullet points found to refine.");
    }
}
