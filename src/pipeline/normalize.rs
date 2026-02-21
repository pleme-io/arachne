/// Normalize a phone number to E.164 format (Brazilian default).
///
/// Strips formatting, applies country code +55 if missing.
pub fn normalize_phone(raw: &str) -> String {
    // Strip everything except digits and leading +
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();

    if digits.is_empty() {
        return String::new();
    }

    // Brazilian phone numbers:
    // - 11 digits: area code (2) + 9 + number (8) → +55XXXXXXXXXXX
    // - 10 digits: area code (2) + number (8) — landline → +55XXXXXXXXXX
    // - 13 digits: 55 + area code (2) + 9 + number (8) — already has country code
    match digits.len() {
        11 => format!("+55{digits}"),
        10 => format!("+55{digits}"),
        13 if digits.starts_with("55") => format!("+{digits}"),
        12 if digits.starts_with("55") => format!("+{digits}"),
        _ => format!("+{digits}"),
    }
}

/// Normalize a name: trim, collapse whitespace, title case.
pub fn normalize_name(raw: &str) -> String {
    raw.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    let rest: String = chars.flat_map(|c| c.to_lowercase()).collect();
                    format!("{upper}{rest}")
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Normalize city slug to a display name.
pub fn slug_to_display(slug: &str) -> String {
    slug.split('-')
        .map(|word| {
            // Keep common prepositions lowercase
            match word {
                "de" | "do" | "da" | "dos" | "das" | "e" => word.to_string(),
                _ => {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => {
                            let upper: String = first.to_uppercase().collect();
                            let rest: String = chars.collect();
                            format!("{upper}{rest}")
                        }
                    }
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_phone_11_digits() {
        assert_eq!(normalize_phone("(11) 99999-1234"), "+5511999991234");
    }

    #[test]
    fn test_normalize_phone_with_country() {
        assert_eq!(normalize_phone("+55 11 99999-1234"), "+5511999991234");
    }

    #[test]
    fn test_normalize_phone_landline() {
        assert_eq!(normalize_phone("1134567890"), "+551134567890");
    }

    #[test]
    fn test_normalize_phone_empty() {
        assert_eq!(normalize_phone(""), "");
        assert_eq!(normalize_phone("no phone"), "");
    }

    #[test]
    fn test_normalize_name() {
        assert_eq!(normalize_name("  maria   silva  "), "Maria Silva");
        assert_eq!(normalize_name("ANA SANTOS"), "Ana Santos");
    }

    #[test]
    fn test_slug_to_display() {
        assert_eq!(slug_to_display("sao-paulo"), "Sao Paulo");
        assert_eq!(slug_to_display("rio-de-janeiro"), "Rio de Janeiro");
    }
}
