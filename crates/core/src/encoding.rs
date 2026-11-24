//! Простые кодировки, которым не нужна отдельная зависимость.

/// Байты → строчный hex.
pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 0x0f)]));
    }
    out
}

/// Hex (любой регистр) → байты. `None`, если строка не hex.
pub(crate) fn hex_decode(text: &str) -> Option<Vec<u8>> {
    fn nibble(c: u8) -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    }
    let (pairs, rest) = text.as_bytes().as_chunks::<2>();
    if !rest.is_empty() {
        return None;
    }
    pairs
        .iter()
        .map(|&[high, low]| Some(nibble(high)? << 4 | nibble(low)?))
        .collect()
}

/// CRC-16/CCITT-FALSE (полином 0x1021, начальное значение 0xFFFF).
pub(crate) fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for &byte in data {
        crc ^= u16::from(byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn hex_known_values() {
        assert_eq!(hex_encode(&[0x00, 0xab, 0xff]), "00abff");
        assert_eq!(hex_decode("00ABff"), Some(vec![0x00, 0xab, 0xff]));
        assert_eq!(hex_decode(""), Some(vec![]));
    }

    #[test]
    fn hex_rejects_garbage() {
        assert_eq!(hex_decode("abc"), None);
        assert_eq!(hex_decode("zz"), None);
        assert_eq!(hex_decode("0x"), None);
    }

    #[test]
    fn crc16_check_value() {
        // Контрольное значение CRC-16/CCITT-FALSE для "123456789".
        assert_eq!(crc16(b"123456789"), 0x29b1);
    }

    proptest! {
        #[test]
        fn hex_roundtrip(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
            prop_assert_eq!(hex_decode(&hex_encode(&bytes)), Some(bytes));
        }
    }
}
