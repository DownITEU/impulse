//! The pseudo-locale: English, accented and a third longer in brackets, so a cut or an English literal shows.

use std::borrow::Cow;

pub fn stretch(text: &str) -> Cow<'_, str> {
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    let mut out = String::with_capacity(text.len() * 2 + 2);
    out.push('[');
    out.extend(text.chars().map(accent));
    out.extend(std::iter::repeat_n('~', letters.div_ceil(3)));
    out.push(']');
    Cow::Owned(out)
}

fn accent(c: char) -> char {
    match c {
        'a' => 'å',
        'c' => 'ç',
        'e' => 'é',
        'i' => 'î',
        'n' => 'ñ',
        'o' => 'ö',
        'u' => 'ü',
        'A' => 'Å',
        'C' => 'Ç',
        'E' => 'É',
        'I' => 'Î',
        'N' => 'Ñ',
        'O' => 'Ö',
        'U' => 'Ü',
        _ => c,
    }
}
