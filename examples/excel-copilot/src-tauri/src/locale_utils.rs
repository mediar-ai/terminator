use std::env;

/// Determine the decimal separator based on system locale
pub fn get_decimal_separator() -> char {
    // Try to get locale from environment variables
    let locale = get_system_locale();
    
    // Countries that use comma (,) as decimal separator
    let comma_countries = [
        "fr", "de", "es", "it", "be", "nl", "pl", "pt", 
        "se", "gr", "ru", "tr", "br", "ar", "id"
    ];
    
    // Extract language code (first 2 characters before any underscore or dash)
    let lang_code = locale.split(['_', '-', '.']).next().unwrap_or("en").to_lowercase();
    
    if comma_countries.contains(&lang_code.as_str()) {
        ','
    } else {
        '.'
    }
}

/// Get system locale from environment variables
fn get_system_locale() -> String {
    // Try different locale environment variables in order of preference
    env::var("LC_NUMERIC")
        .or_else(|_| env::var("LC_ALL"))
        .or_else(|_| env::var("LANG"))
        .or_else(|_| env::var("LANGUAGE"))
        .unwrap_or_else(|_| {
            // On Windows, try to get locale through Windows API
            #[cfg(windows)]
            {
                get_windows_locale().unwrap_or_else(|| "en_US".to_string())
            }
            #[cfg(not(windows))]
            {
                "en_US".to_string()
            }
        })
}

/// Get Windows locale using Windows API
#[cfg(windows)]
fn get_windows_locale() -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    
    unsafe {
        // Get locale name using GetUserDefaultLocaleName
        let mut buffer = [0u16; 85]; // LOCALE_NAME_MAX_LENGTH is 85
        let len = windows::Win32::Globalization::GetUserDefaultLocaleName(
            &mut buffer
        );
        
        if len > 0 {
            let locale_name = OsString::from_wide(&buffer[..len as usize - 1]);
            locale_name.to_str().map(|s| s.to_string())
        } else {
            None
        }
    }
}

/// Format a number according to the system locale
pub fn format_number_for_locale(value: f64) -> String {
    let decimal_separator = get_decimal_separator();
    
    if decimal_separator == ',' {
        // For comma locales, format without thousands separators
        format!("{}", value).replace('.', ",")
    } else {
        // For dot locales, just format normally
        format!("{}", value)
    }
}

/// Normalize a number string for Excel input according to system locale
pub fn normalize_number_for_excel(input: &str) -> String {
    let decimal_separator = get_decimal_separator();
    
    // Remove thousands separators and normalize
    let mut normalized = input
        .replace('+', "")  // Remove plus signs
        .replace(' ', ""); // Remove spaces
    
    if decimal_separator == ',' {
        // For comma locales:
        // - Remove dots used as thousands separators (if they appear before the last comma)
        // - Keep the comma as decimal separator for Excel input
        if let Some(last_comma_pos) = normalized.rfind(',') {
            // Check if there are dots before the last comma (thousands separators)
            let before_comma = &normalized[..last_comma_pos];
            if before_comma.contains('.') {
                // Remove dots used as thousands separators
                normalized = before_comma.replace('.', "") + &normalized[last_comma_pos..];
            }
        }
        // For Excel, we need to convert comma to dot for proper recognition
        normalized = normalized.replace(',', ".");
    } else {
        // For dot locales:
        // - Remove commas used as thousands separators
        // - Keep dots as decimal separators
        
        // Count dots to distinguish between thousands and decimal separators
        let dot_count = normalized.matches('.').count();
        if dot_count > 1 {
            // Multiple dots - likely thousands separators except the last one
            if let Some(last_dot_pos) = normalized.rfind('.') {
                let before_last_dot = &normalized[..last_dot_pos];
                normalized = before_last_dot.replace('.', "") + &normalized[last_dot_pos..];
            }
        }
        // Remove commas (thousands separators in English locales)
        normalized = normalized.replace(',', "");
    }
    
    normalized
}

/// Get current locale information for debugging
pub fn get_locale_info() -> String {
    let locale = get_system_locale();
    let decimal_sep = get_decimal_separator();
    
    format!(
        "System locale: {}, Decimal separator: '{}', Example: 1234{}67 (formatted: {})",
        locale,
        decimal_sep,
        decimal_sep,
        format_number_for_locale(1234.67)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_number_normalization() {
        // Test various number formats
        assert_eq!(normalize_number_for_excel("1,234.56"), "1234.56");
        assert_eq!(normalize_number_for_excel("+2,259.84"), "2259.84");
        assert_eq!(normalize_number_for_excel("-1,458.25"), "-1458.25");
        assert_eq!(normalize_number_for_excel("1234.56"), "1234.56");
    }

    #[test]
    fn test_locale_detection() {
        // Test that the function doesn't panic
        let _ = get_decimal_separator();
        let _ = get_locale_info();
    }
} 