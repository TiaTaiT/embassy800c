// /src/custom_strings.rs
/// Returns the substring after `delimiter`, if present.
pub fn extract_after_delimiter<'a>(input: &'a str, delimiter: &str) -> Option<&'a str> {
    input.split_once(delimiter).map(|(_, suffix)| suffix)
}

/// Returns the substring before `delimiter`, if present.
pub fn extract_before_delimiter<'a>(input: &'a str, delimiter: &str) -> Option<&'a str> {
    input.split_once(delimiter).map(|(prefix, _)| prefix)
}

/// Returns the first substring found between `start_delim` and `end_delim`, if present.
///
/// Example:
/// ```
/// let s = r#"+CPBR: 2,"*105#",129,"0""#;
/// assert_eq!(extract_between_delimiters(s, "\"", "\""), Some("*105#"));
/// ```
pub fn extract_between_delimiters<'a>(
    input: &'a str,
    start_delim: &str,
    end_delim: &str,
) -> Option<&'a str> {
    // Find the start delimiter
    let start_index = input.find(start_delim)?;
    let after_start = &input[start_index + start_delim.len()..];

    // Find the end delimiter after the start delimiter
    let end_index = after_start.find(end_delim)?;
    Some(&after_start[..end_index])
}

/// Takes an immutable &str as input (e.g. "456"),
/// Inserts commas between characters (producing "4,5,6"),
/// Returns the result as a &str,
pub fn separate_chars_by_commas<'a>(
    input: &str,
    output: &'a mut [u8],
) -> Option<&'a str> {
    let bytes = input.as_bytes();
    let len = bytes.len();

    if len == 0 {
        return Some("");
    }

    let required_len = len + (len - 1);
    if output.len() < required_len {
        return None;
    }

    let mut i = 0;
    for (idx, &ch) in bytes.iter().enumerate() {
        output[i] = ch;
        i += 1;
        if idx != len - 1 {
            output[i] = b',';
            i += 1;
        }
    }

    // SAFETY: All bytes come from valid UTF-8 input + ASCII commas
    str::from_utf8(&output[..i]).ok()
}
