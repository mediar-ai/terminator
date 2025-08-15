/// Normalize a string by removing zero-width and special Unicode whitespace characters and lowercasing it.
pub fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| {
            // Remove zero-width and non-breaking spaces, but keep regular spaces
            !matches!(
                *c,
                '\u{200B}' | // zero-width space
                '\u{200C}' | // zero-width non-joiner
                '\u{200D}' | // zero-width joiner
                '\u{00A0}' | // non-breaking space
                '\u{FEFF}' // zero-width no-break space
            )
        })
        .collect::<String>()
        .to_lowercase()
}
