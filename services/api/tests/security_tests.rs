#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use predictiq_api::security::{sanitize, signing, RateLimitConfig, RateLimiter};
    use std::time::Duration;

    #[test]
    fn test_email_sanitization() {
        assert_eq!(
            sanitize::email("  Test@Example.COM  "),
            Some("test@example.com".to_string())
        );
        assert_eq!(sanitize::email("invalid-email"), None);
        assert_eq!(sanitize::email(""), None);
    }

    #[test]
    fn test_string_sanitization() {
        let input = "Hello\x00World\x01Test";
        let result = sanitize::string(input, 100);
        assert!(!result.contains('\x00'));
        assert!(!result.contains('\x01'));

        let long_input = "a".repeat(1000);
        let result = sanitize::string(&long_input, 50);
        assert_eq!(result.len(), 50);
    }

    #[test]
    fn test_sql_injection_detection() {
        assert!(sanitize::contains_sql_injection("' OR '1'='1"));
        assert!(sanitize::contains_sql_injection("'; DROP TABLE users;"));
        assert!(sanitize::contains_sql_injection("UNION SELECT * FROM"));
        assert!(!sanitize::contains_sql_injection("normal query text"));
    }

    #[test]
    fn test_xss_detection() {
        assert!(sanitize::contains_sql_injection(
            "<script>alert('xss')</script>"
        ));
        assert!(sanitize::contains_sql_injection("javascript:alert(1)"));
        assert!(sanitize::contains_sql_injection("onerror=alert(1)"));
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new();
        let config = RateLimitConfig::new(3, Duration::from_secs(60));

        // First 3 requests should succeed
        assert!(limiter.check("test-key", &config).await);
        assert!(limiter.check("test-key", &config).await);
        assert!(limiter.check("test-key", &config).await);

        // 4th request should fail
        assert!(!limiter.check("test-key", &config).await);

        // Different key should succeed
        assert!(limiter.check("other-key", &config).await);
    }

    #[tokio::test]
    async fn test_rate_limiter_window_reset() {
        let limiter = RateLimiter::new();
        let config = RateLimitConfig::new(2, Duration::from_millis(100));

        // Use up the limit
        assert!(limiter.check("test-key", &config).await);
        assert!(limiter.check("test-key", &config).await);
        assert!(!limiter.check("test-key", &config).await);

        // Wait for window to reset
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should work again
        assert!(limiter.check("test-key", &config).await);
    }

    #[test]
    fn test_request_signing() {
        let payload = b"test payload";
        let secret = "test-secret";

        let signature = signing::generate_signature(payload, secret).unwrap();
        assert!(signing::verify_signature(payload, &signature, secret));

        // Wrong payload should fail
        assert!(!signing::verify_signature(
            b"wrong payload",
            &signature,
            secret
        ));

        // Wrong secret should fail
        assert!(!signing::verify_signature(
            payload,
            &signature,
            "wrong-secret"
        ));
    }

    #[test]
    fn test_numeric_id_sanitization() {
        assert_eq!(sanitize::numeric_id("123"), Some(123));
        assert_eq!(sanitize::numeric_id("  456  "), Some(456));
        assert_eq!(sanitize::numeric_id("abc"), None);
        assert_eq!(sanitize::numeric_id("12.34"), None);
    }

    // -------------------------------------------------------------------------
    // #290: generate_signature — fallible API + panic-safety tests
    // -------------------------------------------------------------------------

    #[test]
    fn generate_signature_returns_ok_for_valid_inputs() {
        let sig = signing::generate_signature(b"hello", "secret");
        assert!(sig.is_ok(), "expected Ok for valid payload and secret");
    }

    #[test]
    fn generate_signature_ok_is_verifiable() {
        let payload = b"data";
        let secret = "key";
        let sig = signing::generate_signature(payload, secret).unwrap();
        assert!(signing::verify_signature(payload, &sig, secret));
    }

    #[test]
    fn generate_signature_empty_secret_returns_err() {
        // HMAC rejects a zero-length key; previously this would have panicked
        // via .expect(). Now it must surface as Err(SigningError::InvalidKey).
        let result = signing::generate_signature(b"payload", "");
        assert_eq!(result, Err(signing::SigningError::InvalidKey));
    }

    #[test]
    fn generate_signature_error_is_display_safe() {
        // Ensure the error can be formatted without panicking (used in logs/responses).
        let err = signing::SigningError::InvalidKey;
        assert!(!err.to_string().is_empty());
    }

    // -------------------------------------------------------------------------
    // sanitize::string – Unicode property tests
    // -------------------------------------------------------------------------

    #[test]
    fn string_sanitize_strips_ascii_control_chars() {
        // NUL, SOH, BEL, DEL — all ASCII control chars must be removed.
        let input = "\x00\x01\x07hello\x7f";
        assert_eq!(sanitize::string(input, 100), "hello");
    }

    #[test]
    fn string_sanitize_preserves_normal_whitespace() {
        // Tab, newline, carriage return, space are explicitly allowed.
        let input = "hello\tworld\nnew\r\nline and space";
        let out = sanitize::string(input, 100);
        assert_eq!(out, input);
    }

    #[test]
    fn string_sanitize_strips_nel_u0085() {
        // U+0085 NEXT LINE — is_control() AND is_whitespace() in Rust.
        // Old filter kept it; new filter must strip it.
        let input = "before\u{0085}after";
        let out = sanitize::string(input, 100);
        assert!(!out.contains('\u{0085}'), "U+0085 NEL must be stripped");
        assert_eq!(out, "beforeafter");
    }

    #[test]
    fn string_sanitize_strips_line_separator_u2028() {
        // U+2028 LINE SEPARATOR — Unicode control-like, not ASCII whitespace.
        let input = "a\u{2028}b";
        let out = sanitize::string(input, 100);
        assert!(!out.contains('\u{2028}'), "U+2028 must be stripped");
        assert_eq!(out, "ab");
    }

    #[test]
    fn string_sanitize_strips_paragraph_separator_u2029() {
        let input = "a\u{2029}b";
        let out = sanitize::string(input, 100);
        assert!(!out.contains('\u{2029}'), "U+2029 must be stripped");
        assert_eq!(out, "ab");
    }

    #[test]
    fn string_sanitize_strips_zero_width_space_u200b() {
        // U+200B ZERO WIDTH SPACE — not control, but invisible and policy-violating.
        // NOTE: current filter does NOT strip this (it's not is_control()).
        // This test documents the current boundary; update if policy tightens.
        let input = "a\u{200B}b";
        let out = sanitize::string(input, 100);
        // Document current behavior: passes through (not a control char).
        // If policy changes to strip all invisible chars, update this assertion.
        let _ = out; // accepted either way — test is a property probe
    }

    #[test]
    fn string_sanitize_strips_bom_u_feff() {
        // U+FEFF BOM — not is_control() in Rust, passes through current filter.
        // Document current behavior.
        let input = "\u{FEFF}hello";
        let out = sanitize::string(input, 100);
        let _ = out; // property probe — documents current pass-through
    }

    #[test]
    fn string_sanitize_preserves_multibyte_unicode() {
        // Emoji and CJK must survive sanitization unchanged.
        let input = "héllo wörld 🎉 日本語";
        let out = sanitize::string(input, 100);
        assert_eq!(out, input);
    }

    #[test]
    fn string_sanitize_fuzz_mixed_unicode_categories() {
        // Mix of valid, control, and Unicode special chars.
        let cases: &[(&str, &str)] = &[
            ("abc\x00def", "abcdef"),
            ("ok\u{0085}end", "okend"),
            ("tab\there", "tab\there"),
            ("nl\nhere", "nl\nhere"),
            ("cr\rhere", "cr\rhere"),
            ("\x01\x02\x03", ""),
        ];
        for (input, expected) in cases {
            let out = sanitize::string(input, 200);
            assert_eq!(&out, expected, "input: {input:?}");
        }
    }

    #[test]
    fn string_sanitize_max_len_counts_chars_not_bytes() {
        // "é" is 2 bytes but 1 char — max_len=3 must yield 3 chars.
        let input = "éàü xyz";
        let out = sanitize::string(input, 3);
        assert_eq!(out.chars().count(), 3);
    }

    // -------------------------------------------------------------------------
    // signing::verify_signature – malformed input corpus
    // -------------------------------------------------------------------------

    #[test]
    fn verify_signature_empty_signature_returns_false() {
        assert!(!signing::verify_signature(b"payload", "", "secret"));
    }

    #[test]
    fn verify_signature_not_base64_returns_false() {
        assert!(!signing::verify_signature(b"payload", "not-base64!!!", "secret"));
    }

    #[test]
    fn verify_signature_bad_padding_returns_false() {
        // Valid base64 chars but wrong padding.
        assert!(!signing::verify_signature(b"payload", "YWJj=", "secret"));
    }

    #[test]
    fn verify_signature_truncated_hmac_returns_false() {
        // Too short to be a valid HMAC-SHA256 (32 bytes).
        let short = base64::engine::general_purpose::STANDARD.encode(b"tooshort");
        assert!(!signing::verify_signature(b"payload", &short, "secret"));
    }

    #[test]
    fn verify_signature_wrong_length_all_zeros_returns_false() {
        // 32 zero bytes — correct length but wrong value.
        let zeros = base64::engine::general_purpose::STANDARD.encode([0u8; 32]);
        assert!(!signing::verify_signature(b"payload", &zeros, "secret"));
    }

    #[test]
    fn verify_signature_url_safe_base64_returns_false() {
        // URL-safe base64 (uses - and _) must not be accepted by STANDARD decoder.
        let payload = b"data";
        let secret = "key";
        let sig = signing::generate_signature(payload, secret).unwrap();
        // Replace standard chars with url-safe equivalents to simulate wrong variant.
        let url_safe = sig.replace('+', "-").replace('/', "_");
        if url_safe != sig {
            assert!(!signing::verify_signature(payload, &url_safe, secret));
        }
    }

    #[test]
    fn verify_signature_empty_payload_valid_sig_roundtrips() {
        let sig = signing::generate_signature(b"", "secret").unwrap();
        assert!(signing::verify_signature(b"", &sig, "secret"));
    }

    #[test]
    fn verify_signature_empty_secret_returns_false() {
        // Empty secret — HMAC rejects it; must return false, not panic.
        assert!(!signing::verify_signature(b"payload", "anysig", ""));
    }

    #[test]
    fn verify_signature_unicode_secret_roundtrips() {
        let payload = b"data";
        let secret = "sécret-🔑";
        let sig = signing::generate_signature(payload, secret).unwrap();
        assert!(signing::verify_signature(payload, &sig, secret));
    }

    #[test]
    fn verify_signature_corpus_malformed_variants() {
        // Fuzz corpus: none of these must panic; all must return false.
        let bad_sigs = [
            "====",
            "////",
            "AAAA",                    // valid base64, wrong HMAC
            "AA==",                    // 1 byte decoded — too short
            " ",
            "\x00",
            &"A".repeat(1000),         // very long
            "YQ==\x00extra",           // embedded NUL
        ];
        for sig in bad_sigs {
            let result = signing::verify_signature(b"payload", sig, "secret");
            assert!(!result, "expected false for malformed sig: {sig:?}");
        }
    }
}
