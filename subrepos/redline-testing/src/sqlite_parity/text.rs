pub(crate) fn sanitize_identifier(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

#[cfg(test)]
mod prop_tests {
    use super::sanitize_identifier;
    use proptest::prelude::*;

    proptest! {
        // Invariants that must hold for every possible input string.
        #[test]
        fn sanitize_is_safe_length_preserving_and_idempotent(input in ".*") {
            let out = sanitize_identifier(&input);
            // 1:1 char map: char count is preserved.
            prop_assert_eq!(out.chars().count(), input.chars().count());
            // Every output char is a safe identifier char.
            prop_assert!(out.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
            // Re-sanitizing a sanitized identifier is a no-op.
            prop_assert_eq!(sanitize_identifier(&out), out);
        }
    }
}
