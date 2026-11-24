//! Ключ восстановления.
//!
//! 128 случайных бит и 12 бит контрольной суммы (старшие биты CRC-16/CCITT-FALSE)
//! записываются 28 символами Crockford Base32 в группах по 4:
//! `XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX`.
//!
//! При вводе регистр, пробелы и дефисы не важны, похожие символы прощаются
//! (`O` → `0`, `I` и `L` → `1`). Контрольная сумма ловит опечатки до того, как
//! запускать медленный вывод ключа.

use std::fmt;

use zeroize::{Zeroize, Zeroizing};

use crate::crypto::fill_random;
use crate::encoding::crc16;
use crate::error::{Error, Result};

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const DATA_BITS: usize = 128;
const CHECK_BITS: usize = 12;
const CHARS: usize = (DATA_BITS + CHECK_BITS) / 5;
const GROUP: usize = 4;

pub(crate) const LEN: usize = DATA_BITS / 8;

/// Ключ восстановления. Затирается при удалении, не печатается в журналы.
#[derive(Clone)]
pub struct RecoveryKey(Zeroizing<[u8; LEN]>);

impl RecoveryKey {
    pub(crate) fn generate() -> Self {
        let mut bytes = Zeroizing::new([0u8; LEN]);
        fill_random(bytes.as_mut());
        RecoveryKey(bytes)
    }

    /// Разбирает ключ, введённый пользователем.
    pub fn parse(text: &str) -> Result<Self> {
        let mut values = Zeroizing::new(Vec::with_capacity(CHARS));
        for c in text.chars() {
            if c == '-' || c.is_whitespace() {
                continue;
            }
            values.push(decode_char(c).ok_or(Error::InvalidRecoveryKey)?);
        }
        if values.len() != CHARS {
            return Err(Error::InvalidRecoveryKey);
        }

        let bit = |i: usize| (values[i / 5] >> (4 - i % 5)) & 1;
        let mut data = Zeroizing::new([0u8; LEN]);
        for i in 0..DATA_BITS {
            data[i / 8] |= bit(i) << (7 - i % 8);
        }
        let mut check: u16 = 0;
        for i in DATA_BITS..DATA_BITS + CHECK_BITS {
            check = check << 1 | u16::from(bit(i));
        }
        if check != checksum(&data) {
            return Err(Error::InvalidRecoveryKey);
        }
        Ok(RecoveryKey(data))
    }

    /// Ключ в виде для показа и печати: `XXXX-XXXX-…`.
    pub fn to_display(&self) -> Zeroizing<String> {
        let check = checksum(&self.0);
        let bit = |i: usize| -> u8 {
            if i < DATA_BITS {
                (self.0[i / 8] >> (7 - i % 8)) & 1
            } else {
                // Старшие биты `check` идут первыми; результат 0 или 1.
                u8::from((check >> (DATA_BITS + CHECK_BITS - 1 - i)) & 1 == 1)
            }
        };

        let mut out = Zeroizing::new(String::with_capacity(CHARS + CHARS / GROUP));
        for c in 0..CHARS {
            if c > 0 && c % GROUP == 0 {
                out.push('-');
            }
            let mut value = 0u8;
            for i in c * 5..c * 5 + 5 {
                value = value << 1 | bit(i);
            }
            out.push(char::from(ALPHABET[usize::from(value)]));
        }
        out
    }

    pub(crate) fn as_bytes(&self) -> &[u8; LEN] {
        &self.0
    }
}

impl fmt::Debug for RecoveryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryKey(..)")
    }
}

impl Zeroize for RecoveryKey {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

fn checksum(data: &[u8; LEN]) -> u16 {
    crc16(data) >> (16 - CHECK_BITS)
}

fn decode_char(c: char) -> Option<u8> {
    let c = match c.to_ascii_uppercase() {
        'O' => '0',
        'I' | 'L' => '1',
        c => c,
    };
    let c = u8::try_from(c).ok()?;
    ALPHABET
        .iter()
        .position(|&a| a == c)
        .and_then(|i| u8::try_from(i).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn display_format() {
        let key = RecoveryKey::generate();
        let text = key.to_display();
        assert_eq!(text.len(), 34);
        let groups: Vec<&str> = text.split('-').collect();
        assert_eq!(groups.len(), 7);
        assert!(groups.iter().all(|g| g.len() == 4));
        assert!(text.bytes().all(|b| b == b'-' || ALPHABET.contains(&b)));
    }

    #[test]
    fn known_value() {
        let key = RecoveryKey(Zeroizing::new([0u8; LEN]));
        // 128 нулевых бит, затем 12 бит контрольной суммы.
        let check = checksum(&[0u8; LEN]);
        let text = key.to_display();
        assert!(text.starts_with("0000-0000-0000-0000-0000-0000-0"));
        let parsed = RecoveryKey::parse(&text).unwrap();
        assert_eq!(parsed.as_bytes(), &[0u8; LEN]);
        assert_eq!(checksum(parsed.as_bytes()), check);
    }

    #[test]
    fn forgiving_input() {
        let key = RecoveryKey::generate();
        let text = key.to_display().to_string();
        let sloppy = text
            .to_lowercase()
            .replace('-', " ")
            .replace('0', "o")
            .replace('1', "l");
        let parsed = RecoveryKey::parse(&format!("  {sloppy}\n")).unwrap();
        assert_eq!(parsed.as_bytes(), key.as_bytes());
    }

    #[test]
    fn rejects_bad_input() {
        let text = RecoveryKey::generate().to_display().to_string();
        assert!(RecoveryKey::parse("").is_err());
        assert!(RecoveryKey::parse(&text[..text.len() - 1]).is_err());
        assert!(RecoveryKey::parse(&format!("{text}0")).is_err());
        assert!(RecoveryKey::parse(&text.replacen(|c: char| c != '-', "U", 1)).is_err());
        assert!(RecoveryKey::parse(&text.replacen(|c: char| c != '-', "Ж", 1)).is_err());
    }

    #[test]
    fn single_typos_are_mostly_caught() {
        // 12 бит контрольной суммы: случайная опечатка проходит с вероятностью 1/4096.
        let mut caught = 0;
        let mut total = 0;
        for _ in 0..20 {
            let text = RecoveryKey::generate().to_display().to_string();
            for (pos, original) in text.char_indices().filter(|(_, c)| *c != '-') {
                for &replacement in ALPHABET {
                    let replacement = char::from(replacement);
                    if replacement == original {
                        continue;
                    }
                    let mut typo = text.clone();
                    typo.replace_range(pos..pos + 1, &replacement.to_string());
                    total += 1;
                    if RecoveryKey::parse(&typo).is_err() {
                        caught += 1;
                    }
                }
            }
        }
        assert!(caught * 1000 >= total * 995, "поймано {caught} из {total}");
    }

    #[test]
    fn debug_hides_key() {
        assert_eq!(format!("{:?}", RecoveryKey::generate()), "RecoveryKey(..)");
    }

    proptest! {
        #[test]
        fn roundtrip(bytes in any::<[u8; LEN]>()) {
            let key = RecoveryKey(Zeroizing::new(bytes));
            let parsed = RecoveryKey::parse(&key.to_display()).unwrap();
            prop_assert_eq!(parsed.as_bytes(), &bytes);
        }
    }
}
