//! Tests for UTF-8 character support in selectors (Chinese, Japanese, Korean, etc.)
//!
//! This test file verifies that Terminator correctly handles non-ASCII characters
//! in selector strings, including Chinese characters, emoji, and other UTF-8 text.
//!
//! Related issue: #299

use crate::Selector;

#[test]
fn test_chinese_characters_in_role_name_selector() {
    // Test Chinese characters in role|name format
    let selector_str = "role:Button|name:提交"; // "Submit" in Chinese
    let selector = Selector::from(selector_str);

    match selector {
        Selector::Role { role, name } => {
            assert_eq!(role, "Button");
            assert_eq!(name, Some("提交".to_string()));
        }
        _ => panic!("Expected Role selector, got: {selector:?}"),
    }
}

#[test]
fn test_japanese_characters_in_name_selector() {
    // Test Japanese characters (Hiragana)
    let selector_str = "name:こんにちは"; // "Hello" in Japanese
    let selector = Selector::from(selector_str);

    match selector {
        Selector::Name(name) => {
            assert_eq!(name, "こんにちは");
        }
        _ => panic!("Expected Name selector, got: {selector:?}"),
    }
}

#[test]
fn test_korean_characters_in_text_selector() {
    // Test Korean characters (Hangul)
    let selector_str = "text:안녕하세요"; // "Hello" in Korean
    let selector = Selector::from(selector_str);

    match selector {
        Selector::Text(text) => {
            assert_eq!(text, "안녕하세요");
        }
        _ => panic!("Expected Text selector, got: {selector:?}"),
    }
}

#[test]
fn test_emoji_in_selector() {
    // Test emoji characters
    let selector_str = "role:Button|name:保存 💾"; // Save with floppy disk emoji
    let selector = Selector::from(selector_str);

    match selector {
        Selector::Role { role, name } => {
            assert_eq!(role, "Button");
            assert_eq!(name, Some("保存 💾".to_string()));
        }
        _ => panic!("Expected Role selector, got: {selector:?}"),
    }
}

#[test]
fn test_mixed_language_selector() {
    // Test mixed English and Chinese
    let selector_str = "role:Window|name:Settings 设置";
    let selector = Selector::from(selector_str);

    match selector {
        Selector::Role { role, name } => {
            assert_eq!(role, "Window");
            assert_eq!(name, Some("Settings 设置".to_string()));
        }
        _ => panic!("Expected Role selector, got: {selector:?}"),
    }
}

#[test]
fn test_chinese_in_chained_selector() {
    // Test Chinese characters in chained selectors
    let selector_str = "role:Window|name:主窗口 >> role:Button|name:确定";
    let selector = Selector::from(selector_str);

    match selector {
        Selector::Chain(parts) => {
            assert_eq!(parts.len(), 2);

            // First part
            if let Selector::Role { role, name } = &parts[0] {
                assert_eq!(role, "Window");
                assert_eq!(name, &Some("主窗口".to_string())); // "Main Window"
            } else {
                panic!("Expected first part to be Role selector");
            }

            // Second part
            if let Selector::Role { role, name } = &parts[1] {
                assert_eq!(role, "Button");
                assert_eq!(name, &Some("确定".to_string())); // "OK"
            } else {
                panic!("Expected second part to be Role selector");
            }
        }
        _ => panic!("Expected Chain selector, got: {selector:?}"),
    }
}

#[test]
fn test_arabic_rtl_text() {
    // Test Arabic (right-to-left) text
    let selector_str = "name:مرحبا"; // "Hello" in Arabic
    let selector = Selector::from(selector_str);

    match selector {
        Selector::Name(name) => {
            assert_eq!(name, "مرحبا");
        }
        _ => panic!("Expected Name selector, got: {selector:?}"),
    }
}

#[test]
fn test_cyrillic_characters() {
    // Test Cyrillic characters (Russian)
    let selector_str = "role:Button|name:Привет"; // "Hello" in Russian
    let selector = Selector::from(selector_str);

    match selector {
        Selector::Role { role, name } => {
            assert_eq!(role, "Button");
            assert_eq!(name, Some("Привет".to_string()));
        }
        _ => panic!("Expected Role selector, got: {selector:?}"),
    }
}

#[test]
fn test_special_unicode_characters() {
    // Test various Unicode special characters
    let test_cases = vec![
        ("name:文本编辑器", "文本编辑器"), // Chinese "Text Editor"
        ("name:ファイル", "ファイル"),     // Japanese "File"
        ("name:파일", "파일"),            // Korean "File"
        ("name:Файл", "Файл"),           // Russian "File"
        ("name:Αρχείο", "Αρχείο"),        // Greek "File"
        ("name:ملف", "ملف"),             // Arabic "File"
    ];

    for (selector_str, expected_name) in test_cases {
        let selector = Selector::from(selector_str);
        match selector {
            Selector::Name(name) => {
                assert_eq!(name, expected_name, "Failed for selector: {selector_str}");
            }
            _ => panic!("Expected Name selector for '{selector_str}', got: {selector:?}"),
        }
    }
}

#[test]
fn test_utf8_byte_length_vs_char_length() {
    // Verify that string slicing works correctly with multi-byte UTF-8 characters
    // This tests the internal string handling in selector parsing
    let selector_str = "role:你好"; // Chinese "Hello" - each character is 3 bytes in UTF-8
    let selector = Selector::from(selector_str);

    match selector {
        Selector::Role { role, name } => {
            assert_eq!(role, "你好");
            assert_eq!(name, None);
            // Verify byte length != character length
            assert_eq!(role.len(), 6); // 2 Chinese chars * 3 bytes each
            assert_eq!(role.chars().count(), 2); // 2 characters
        }
        _ => panic!("Expected Role selector, got: {selector:?}"),
    }
}

#[test]
fn test_nativeid_with_chinese() {
    // Test NativeId selector with Chinese characters
    let selector_str = "nativeid:按钮_提交";
    let selector = Selector::from(selector_str);

    match selector {
        Selector::NativeId(id) => {
            assert_eq!(id, "按钮_提交");
        }
        _ => panic!("Expected NativeId selector, got: {selector:?}"),
    }
}

#[test]
fn test_classname_with_unicode() {
    // Test ClassName selector with Unicode
    let selector_str = "classname:UI控件";
    let selector = Selector::from(selector_str);

    match selector {
        Selector::ClassName(class) => {
            assert_eq!(class, "UI控件");
        }
        _ => panic!("Expected ClassName selector, got: {selector:?}"),
    }
}

#[test]
fn test_contains_with_chinese() {
    // Test contains: prefix with Chinese characters
    let selector_str = "role:Button|contains:提交";
    let selector = Selector::from(selector_str);

    match selector {
        Selector::Role { role, name } => {
            assert_eq!(role, "Button");
            assert_eq!(name, Some("提交".to_string()));
        }
        _ => panic!("Expected Role selector, got: {selector:?}"),
    }
}
