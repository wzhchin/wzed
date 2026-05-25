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
