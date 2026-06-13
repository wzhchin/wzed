use std::path::Path;

use encoding_rs::Encoding;

pub(crate) fn detect_encoding(bytes: &[u8]) -> &'static Encoding {
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(bytes, true);
    detector.guess(None, true)
}

pub(crate) fn decode_bytes(bytes: &[u8], encoding: &'static Encoding) -> String {
    let (cow, _encoding_used, _had_errors) = encoding.decode(bytes);
    cow.into_owned()
}

// Encode text back into the file's encoding for saving. Returns Err when the
// text contains characters the encoding cannot represent — we must not write a
// silently-corrupted file (encoding_rs would otherwise substitute numeric
// character references), so the caller aborts the save and surfaces the error.
pub(crate) fn encode_string(text: &str, encoding: &'static Encoding) -> Result<Vec<u8>, String> {
    let (cow, _encoding_used, had_errors) = encoding.encode(text);
    if had_errors {
        return Err(format!(
            "text contains characters that cannot be encoded in {}",
            encoding.name()
        ));
    }
    Ok(cow.into_owned())
}

pub(crate) fn read_file_with_detection(path: &Path) -> std::io::Result<(String, &'static Encoding)> {
    let bytes = std::fs::read(path)?;
    let encoding = detect_encoding(&bytes);
    let text = decode_bytes(&bytes, encoding);
    Ok((text, encoding))
}

pub(crate) fn read_file_as_encoding(path: &Path, encoding: &'static Encoding) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(decode_bytes(&bytes, encoding))
}

pub(crate) fn encoding_label(encoding: &'static Encoding) -> &'static str {
    encoding.name()
}

pub(crate) fn encoding_from_label(label: &str) -> Option<&'static Encoding> {
    Encoding::for_label(label.as_bytes())
}

pub(crate) const SUPPORTED_ENCODINGS: &[&str] = &[
    "UTF-8",
    "UTF-16LE",
    "UTF-16BE",
    "GBK",
    "GB18030",
    "Big5",
    "Shift_JIS",
    "EUC-JP",
    "EUC-KR",
    "ISO-8859-1",
    "ISO-8859-2",
    "ISO-8859-5",
    "ISO-8859-15",
    "Windows-1252",
    "Windows-1251",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_encoding_utf8() {
        let text = "Hello, world! 你好世界";
        let encoding = detect_encoding(text.as_bytes());
        assert_eq!(encoding.name(), "UTF-8");
    }

    #[test]
    fn test_detect_encoding_ascii() {
        let text = b"plain ascii text";
        let encoding = detect_encoding(text);
        assert_eq!(encoding.name(), "UTF-8");
    }

    #[test]
    fn test_decode_bytes_utf8() {
        let text = "Hello 你好";
        let bytes = text.as_bytes();
        let decoded = decode_bytes(bytes, encoding_rs::UTF_8);
        assert_eq!(decoded, text);
    }

    #[test]
    fn test_encode_string_utf8_passthrough() {
        let text = "Hello 你好";
        let bytes = encode_string(text, encoding_rs::UTF_8).unwrap();
        assert_eq!(bytes, text.as_bytes());
    }

    #[test]
    fn test_encode_string_gbk_roundtrip() {
        let text = "你好，世界 hello";
        let bytes = encode_string(text, encoding_rs::GBK).unwrap();
        // Decoding the encoded bytes must reproduce the original text.
        assert_eq!(decode_bytes(&bytes, encoding_rs::GBK), text);
    }

    #[test]
    fn test_encode_string_unencodable_is_err() {
        // A Latin-1-style encoding cannot represent CJK characters; the save
        // must abort rather than silently substitute characters.
        let latin1 = encoding_from_label("ISO-8859-1").expect("ISO-8859-1 resolves");
        let result = encode_string("你", latin1);
        assert!(result.is_err(), "expected Err for unencodable char");
    }

    #[test]
    fn test_encoding_label_roundtrip() {
        for &label in SUPPORTED_ENCODINGS {
            let enc = encoding_from_label(label);
            assert!(enc.is_some(), "failed to resolve label: {label}");
            let resolved = enc.unwrap();
            // encoding_from_label should roundtrip: resolving the canonical name again should
            // yield the same encoding
            assert_eq!(
                encoding_from_label(resolved.name()),
                Some(resolved),
                "roundtrip failed for {label} -> {}",
                resolved.name(),
            );
        }
    }

    #[test]
    fn test_encoding_from_label_case_insensitive() {
        assert!(encoding_from_label("utf-8").is_some());
        assert!(encoding_from_label("UTF-8").is_some());
        assert!(encoding_from_label("utf8").is_some());
    }

    #[test]
    fn test_encoding_from_label_unknown() {
        assert!(encoding_from_label("not-a-real-encoding").is_none());
        assert!(encoding_from_label("").is_none());
    }
}
