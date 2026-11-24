//! Генератор паролей и парольных фраз, оценка надёжности.
//!
//! Все случайные выборы делаются системным генератором без смещения
//! ([`crate::crypto::random_below`]).

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::crypto::random_below;
use crate::error::{Invalid, Result};

const LOWERCASE: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPERCASE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &str = "0123456789";
/// Без кавычек, обратной косой черты и обратного апострофа: они ломают формы
/// и командные строки.
const SYMBOLS: &str = "!#$%&()*+,-./:;<=>?@[]^_{|}~";
/// Символы, которые легко спутать при чтении с экрана или с бумаги.
const SIMILAR: &str = "0Oo1lI|";

pub const PASSWORD_MIN_LENGTH: usize = 4;
pub const PASSWORD_MAX_LENGTH: usize = 128;
pub const PASSPHRASE_MIN_WORDS: usize = 3;
pub const PASSPHRASE_MAX_WORDS: usize = 20;
const SEPARATOR_MAX_LENGTH: usize = 8;

/// Сколько первых символов пароля учитывает оценка надёжности: zxcvbn
/// замедляется на длинных строках, а длиннее пароль сильнее не станет.
const STRENGTH_MAX_CHARS: usize = 100;

/// Словарь EFF для парольных фраз: 7776 слов (≈12,9 бита на слово).
/// Источник и лицензия — `wordlists/README.md`.
const EFF_WORDLIST: &str = include_str!("../wordlists/eff_large_wordlist.txt");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PasswordOptions {
    pub length: usize,
    pub lowercase: bool,
    pub uppercase: bool,
    pub digits: bool,
    pub symbols: bool,
    /// Не использовать похожие символы (`0`/`O`, `1`/`l`/`I` и т. п.).
    pub avoid_similar: bool,
}

impl Default for PasswordOptions {
    fn default() -> Self {
        Self {
            length: 20,
            lowercase: true,
            uppercase: true,
            digits: true,
            symbols: true,
            avoid_similar: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PassphraseOptions {
    pub words: usize,
    pub separator: String,
    /// Каждое слово с заглавной буквы.
    pub capitalize: bool,
    /// Добавить цифру к одному из слов (многие сайты требуют цифру).
    pub number: bool,
}

impl Default for PassphraseOptions {
    fn default() -> Self {
        Self {
            words: 6,
            separator: "-".into(),
            capitalize: true,
            number: true,
        }
    }
}

/// Оценка надёжности пароля.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Strength {
    /// 0 — очень слабый … 4 — сильный.
    pub score: u8,
    /// Десятичный логарифм оценки числа попыток для подбора.
    pub guesses_log10: f64,
}

/// Случайный пароль. В нём есть хотя бы один символ из каждого выбранного набора.
pub fn generate_password(options: &PasswordOptions) -> Result<Zeroizing<String>> {
    if !(PASSWORD_MIN_LENGTH..=PASSWORD_MAX_LENGTH).contains(&options.length) {
        return Err(Invalid::OutOfRange {
            min: PASSWORD_MIN_LENGTH,
            max: PASSWORD_MAX_LENGTH,
        }
        .into());
    }
    let sets: Vec<Vec<char>> = [
        (options.lowercase, LOWERCASE),
        (options.uppercase, UPPERCASE),
        (options.digits, DIGITS),
        (options.symbols, SYMBOLS),
    ]
    .into_iter()
    .filter(|(enabled, _)| *enabled)
    .map(|(_, set)| {
        set.chars()
            .filter(|c| !(options.avoid_similar && SIMILAR.contains(*c)))
            .collect()
    })
    .collect();
    if sets.is_empty() {
        return Err(Invalid::NoCharacterSets.into());
    }
    let all: Vec<char> = sets.concat();

    let mut chars = Zeroizing::new(Vec::with_capacity(options.length));
    chars.extend(sets.iter().map(|set| pick(set)));
    while chars.len() < options.length {
        chars.push(pick(&all));
    }
    shuffle(&mut chars);

    let mut password = Zeroizing::new(String::with_capacity(options.length));
    password.extend(chars.iter());
    Ok(password)
}

/// Случайная парольная фраза из словаря EFF.
pub fn generate_passphrase(options: &PassphraseOptions) -> Result<Zeroizing<String>> {
    if !(PASSPHRASE_MIN_WORDS..=PASSPHRASE_MAX_WORDS).contains(&options.words) {
        return Err(Invalid::OutOfRange {
            min: PASSPHRASE_MIN_WORDS,
            max: PASSPHRASE_MAX_WORDS,
        }
        .into());
    }
    if options.separator.chars().count() > SEPARATOR_MAX_LENGTH {
        return Err(Invalid::TooLong {
            field: "separator",
            max: SEPARATOR_MAX_LENGTH,
        }
        .into());
    }

    let words = wordlist();
    let with_number = options
        .number
        .then(|| (index_below(options.words), random_below(10)));

    let mut phrase = Zeroizing::new(String::new());
    for i in 0..options.words {
        if i > 0 {
            phrase.push_str(&options.separator);
        }
        let word = pick(words);
        if options.capitalize {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                phrase.extend(first.to_uppercase());
                phrase.push_str(chars.as_str());
            }
        } else {
            phrase.push_str(word);
        }
        if let Some((position, digit)) = with_number
            && position == i
        {
            phrase.push(char::from_digit(digit, 10).expect("цифра от 0 до 9"));
        }
    }
    Ok(phrase)
}

/// Оценивает надёжность пароля. `user_inputs` — слова, которые легко угадать
/// для этой записи (название, логин, сайт): пароль из них считается слабым.
pub fn password_strength(password: &str, user_inputs: &[&str]) -> Strength {
    let truncated: Zeroizing<String> =
        Zeroizing::new(password.chars().take(STRENGTH_MAX_CHARS).collect());
    let entropy = zxcvbn::zxcvbn(&truncated, user_inputs);
    Strength {
        score: entropy.score().into(),
        guesses_log10: entropy.guesses_log10(),
    }
}

fn wordlist() -> &'static [&'static str] {
    static WORDS: OnceLock<Vec<&'static str>> = OnceLock::new();
    WORDS.get_or_init(|| {
        EFF_WORDLIST
            .lines()
            .filter_map(|line| line.split('\t').nth(1))
            .map(str::trim)
            .filter(|word| !word.is_empty())
            .collect()
    })
}

fn index_below(n: usize) -> usize {
    let n = u32::try_from(n).expect("наборы и списки короче 2^32");
    random_below(n) as usize
}

fn pick<T: Copy>(items: &[T]) -> T {
    items[index_below(items.len())]
}

/// Перемешивание Фишера — Йейтса.
fn shuffle<T>(items: &mut [T]) {
    for i in (1..items.len()).rev() {
        items.swap(i, index_below(i + 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use std::collections::HashSet;

    fn has_any(password: &str, set: &str) -> bool {
        password.chars().any(|c| set.contains(c))
    }

    #[test]
    fn default_password() {
        let password = generate_password(&PasswordOptions::default()).unwrap();
        assert_eq!(password.chars().count(), 20);
        for set in [LOWERCASE, UPPERCASE, DIGITS, SYMBOLS] {
            assert!(has_any(&password, set), "{set}");
        }
    }

    #[test]
    fn every_enabled_set_is_used_even_when_short() {
        for _ in 0..200 {
            let password = generate_password(&PasswordOptions {
                length: 4,
                ..Default::default()
            })
            .unwrap();
            for set in [LOWERCASE, UPPERCASE, DIGITS, SYMBOLS] {
                assert!(has_any(&password, set));
            }
        }
    }

    #[test]
    fn only_selected_sets() {
        let password = generate_password(&PasswordOptions {
            length: 64,
            lowercase: false,
            uppercase: false,
            symbols: false,
            ..Default::default()
        })
        .unwrap();
        assert!(password.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn avoid_similar() {
        for _ in 0..50 {
            let password = generate_password(&PasswordOptions {
                length: 128,
                avoid_similar: true,
                ..Default::default()
            })
            .unwrap();
            assert!(!has_any(&password, SIMILAR), "{}", password.as_str());
        }
    }

    #[test]
    fn symbols_are_safe() {
        for bad in ['"', '\'', '\\', '`', ' '] {
            assert!(!SYMBOLS.contains(bad));
        }
    }

    #[test]
    fn invalid_password_options() {
        let too_short = PasswordOptions {
            length: PASSWORD_MIN_LENGTH - 1,
            ..Default::default()
        };
        let too_long = PasswordOptions {
            length: PASSWORD_MAX_LENGTH + 1,
            ..Default::default()
        };
        let no_sets = PasswordOptions {
            lowercase: false,
            uppercase: false,
            digits: false,
            symbols: false,
            ..Default::default()
        };
        assert!(matches!(
            generate_password(&too_short),
            Err(Error::Invalid(Invalid::OutOfRange { .. }))
        ));
        assert!(matches!(
            generate_password(&too_long),
            Err(Error::Invalid(Invalid::OutOfRange { .. }))
        ));
        assert!(matches!(
            generate_password(&no_sets),
            Err(Error::Invalid(Invalid::NoCharacterSets))
        ));
    }

    #[test]
    fn passwords_are_distributed_evenly() {
        // 26 строчных букв, 20 000 символов: каждая должна встречаться ~770 раз.
        let password = (0..100)
            .map(|_| {
                generate_password(&PasswordOptions {
                    length: 128,
                    uppercase: false,
                    digits: false,
                    symbols: false,
                    ..Default::default()
                })
                .unwrap()
                .to_string()
            })
            .collect::<String>();
        let total = password.chars().count();
        for letter in LOWERCASE.chars() {
            let count = password.chars().filter(|&c| c == letter).count();
            let expected = total / 26;
            assert!(
                count > expected * 7 / 10 && count < expected * 13 / 10,
                "{letter}: {count} из {total}"
            );
        }
    }

    #[test]
    fn passwords_do_not_repeat() {
        let set: HashSet<String> = (0..1000)
            .map(|_| {
                generate_password(&PasswordOptions::default())
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(set.len(), 1000);
    }

    #[test]
    fn wordlist_is_complete() {
        let words = wordlist();
        assert_eq!(words.len(), 7776);
        assert_eq!(words.first(), Some(&"abacus"));
        assert_eq!(words.last(), Some(&"zoom"));
        assert_eq!(words.iter().collect::<HashSet<_>>().len(), 7776);
    }

    #[test]
    fn default_passphrase() {
        let phrase = generate_passphrase(&PassphraseOptions::default()).unwrap();
        let words: Vec<&str> = phrase.split('-').collect();
        // В словаре EFF есть слова с дефисом (t-shirt), поэтому частей может быть больше.
        assert!(words.len() >= 6, "{}", phrase.as_str());
        assert!(phrase.chars().any(|c| c.is_ascii_digit()));
        assert!(phrase.starts_with(|c: char| c.is_ascii_uppercase()));
    }

    #[test]
    fn plain_passphrase() {
        let phrase = generate_passphrase(&PassphraseOptions {
            words: 4,
            separator: " ".into(),
            capitalize: false,
            number: false,
        })
        .unwrap();
        let words: Vec<&str> = phrase.split(' ').collect();
        assert_eq!(words.len(), 4);
        for word in words {
            assert!(wordlist().contains(&word), "{word}");
        }
    }

    #[test]
    fn invalid_passphrase_options() {
        for options in [
            PassphraseOptions {
                words: PASSPHRASE_MIN_WORDS - 1,
                ..Default::default()
            },
            PassphraseOptions {
                words: PASSPHRASE_MAX_WORDS + 1,
                ..Default::default()
            },
            PassphraseOptions {
                separator: "-".repeat(SEPARATOR_MAX_LENGTH + 1),
                ..Default::default()
            },
        ] {
            assert!(generate_passphrase(&options).is_err(), "{options:?}");
        }
    }

    #[test]
    fn strength() {
        assert_eq!(password_strength("password", &[]).score, 0);
        assert_eq!(password_strength("", &[]).score, 0);
        let strong = generate_password(&PasswordOptions::default()).unwrap();
        assert_eq!(password_strength(&strong, &[]).score, 4);
        let phrase = generate_passphrase(&PassphraseOptions::default()).unwrap();
        assert_eq!(password_strength(&phrase, &[]).score, 4);
        // Пароль из логина пользователя слабый.
        let weak = password_strength("ivan.petrov1990", &["ivan.petrov1990"]);
        assert!(weak.score <= 1);
        // Длинный пароль оценивается по первым STRENGTH_MAX_CHARS символам.
        let long = random_text(STRENGTH_MAX_CHARS * 10);
        let head: String = long.chars().take(STRENGTH_MAX_CHARS).collect();
        assert_eq!(password_strength(&long, &[]), password_strength(&head, &[]));
    }

    fn random_text(len: usize) -> String {
        generate_password(&PasswordOptions {
            length: PASSWORD_MAX_LENGTH,
            ..Default::default()
        })
        .unwrap()
        .chars()
        .cycle()
        .take(len)
        .collect()
    }

    /// Оценка вызывается на каждое нажатие клавиши, поэтому должна быть быстрой:
    /// `cargo test -p xonkypass-core --release -- --ignored --nocapture strength_timing`
    #[test]
    #[ignore = "замер времени, запускается вручную"]
    fn strength_timing() {
        for len in [12, 20, 32, 50, 64, 100] {
            let password: String = (0..len)
                .map(|_| {
                    generate_password(&PasswordOptions::default())
                        .unwrap()
                        .as_str()[..1]
                        .to_owned()
                })
                .collect();
            let start = std::time::Instant::now();
            let strength = password_strength(&password, &[]);
            println!(
                "{len:>3} символов: {:?}, оценка {}",
                start.elapsed(),
                strength.score
            );
        }
    }
}
