//! Decoding of process output that is not UTF-8. On Windows console programs (the built-ins of `cmd`, `dir`,
//! `ipconfig`, ...) write in the OEM code page of the system - 866 on a Russian Windows - so Cyrillic would
//! otherwise show up as replacement characters.

/// Decode `bytes` that are not valid UTF-8: with the OEM code page on Windows, lossily elsewhere.
pub fn decode_non_utf8(bytes: &[u8]) -> String {
    #[cfg(windows)]
    {
        if let Some(s) = decode_oem(bytes) {
            return s;
        }
    }
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(windows)]
fn decode_oem(bytes: &[u8]) -> Option<String> {
    unsafe extern "system" {
        fn MultiByteToWideChar(code_page: u32, flags: u32, src: *const u8, src_len: i32, dst: *mut u16, dst_len: i32) -> i32;
    }
    const CP_OEMCP: u32 = 1;
    if bytes.is_empty() {
        return Some(String::new());
    }
    let len = i32::try_from(bytes.len()).ok()?;
    // SAFETY: `src` points to `len` readable bytes; the first call only asks for the required size (dst is null),
    // the second writes at most `n` u16 into a buffer of exactly that size.
    let n = unsafe { MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), len, std::ptr::null_mut(), 0) };
    if n <= 0 {
        return None;
    }
    let mut buf = vec![0u16; n as usize];
    let m = unsafe { MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), len, buf.as_mut_ptr(), n) };
    (m > 0).then(|| String::from_utf16_lossy(&buf[..m as usize]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_and_empty_pass_through() {
        assert_eq!(decode_non_utf8(b""), "");
        assert_eq!(decode_non_utf8(b"plain text 123\r\n"), "plain text 123\r\n");
    }

    #[test]
    fn high_bytes_never_produce_replacement_characters_on_windows() {
        let s = decode_non_utf8(&[0xa7, 0xad, 0xa0, 0xe7, 0xa5, 0xad, 0xa8, 0xa5]);
        assert_eq!(s.chars().count(), 8);
        if cfg!(windows) {
            assert!(!s.contains('\u{fffd}'), "{s:?}");
        }
    }
}
