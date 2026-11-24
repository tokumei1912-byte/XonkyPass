//! Симметричное шифрование: XChaCha20-Poly1305 со случайным nonce.
//!
//! Формат шифртекста: `nonce (24 байта) ‖ шифртекст ‖ тег (16 байт)`.
//! Каждый шифртекст привязан к месту хранения через связанные данные ([`Aad`]),
//! поэтому его нельзя незаметно перенести в другую запись, поле или хранилище.

use std::fmt;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

pub(crate) const KEY_LEN: usize = 32;
pub(crate) const NONCE_LEN: usize = 24;
pub(crate) const TAG_LEN: usize = 16;

/// Метка в начале связанных данных: отличает XonkyPass от любых других форматов.
const AAD_LABEL: &[u8] = b"XonkyPass";

/// Симметричный ключ 256 бит. Затирается при удалении, не печатается в журналы.
pub(crate) struct Key(Zeroizing<[u8; KEY_LEN]>);

impl Key {
    pub(crate) fn random() -> Self {
        let mut bytes = Zeroizing::new([0u8; KEY_LEN]);
        fill_random(bytes.as_mut());
        Key(bytes)
    }

    pub(crate) fn from_bytes(bytes: Zeroizing<[u8; KEY_LEN]>) -> Self {
        Key(bytes)
    }

    pub(crate) fn bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new_from_slice(self.0.as_ref()).expect("ключ всегда 32 байта")
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(..)")
    }
}

/// Заполняет буфер случайными байтами из системного генератора.
///
/// Отказ системного генератора на Windows 10+ практически невозможен; продолжать
/// работу без случайности нельзя, поэтому это паника, а не ошибка.
pub(crate) fn fill_random(buf: &mut [u8]) {
    getrandom::fill(buf).expect("системный генератор случайных чисел недоступен");
}

/// Случайное число от 0 до `n - 1` без смещения (отбрасывание «хвоста»).
pub(crate) fn random_below(n: u32) -> u32 {
    assert!(n > 0, "диапазон не может быть пустым");
    // Наибольшее кратное `n` число, не превышающее 2^32 - 1.
    let zone = u32::MAX - (u32::MAX % n);
    loop {
        let x = getrandom::u32().expect("системный генератор случайных чисел недоступен");
        if x < zone {
            return x % n;
        }
    }
}

/// Не сошлась проверка подлинности: неверный ключ, чужие связанные данные
/// или изменённый шифртекст. Причины намеренно неразличимы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AuthError;

/// Шифрует `plaintext` со свежим случайным nonce.
pub(crate) fn seal(key: &Key, aad: &Aad, plaintext: &[u8]) -> Vec<u8> {
    let mut nonce = [0u8; NONCE_LEN];
    fill_random(&mut nonce);
    let nonce = XNonce::from(nonce);
    let ciphertext = key
        .cipher()
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad: aad.as_bytes(),
            },
        )
        .expect("XChaCha20-Poly1305 шифрует сообщения любой разумной длины");

    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    out
}

/// Расшифровывает результат [`seal`] и проверяет его подлинность.
pub(crate) fn open(key: &Key, aad: &Aad, data: &[u8]) -> Result<Zeroizing<Vec<u8>>, AuthError> {
    if data.len() < NONCE_LEN + TAG_LEN {
        return Err(AuthError);
    }
    let (nonce, ciphertext) = data.split_at(NONCE_LEN);
    let nonce = <&XNonce>::try_from(nonce).map_err(|_| AuthError)?;
    key.cipher()
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad: aad.as_bytes(),
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| AuthError)
}

/// Связанные данные (AAD): `"XonkyPass" ‖ контекст ‖ поля`.
///
/// Контекст и каждое поле записываются с длиной (u32 little-endian) впереди,
/// поэтому разные наборы полей никогда не дают одинаковые байты.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Aad(Vec<u8>);

impl Aad {
    pub(crate) fn new(context: &str) -> Self {
        Aad(AAD_LABEL.to_vec()).part(context.as_bytes())
    }

    pub(crate) fn part(mut self, bytes: &[u8]) -> Self {
        let len = u32::try_from(bytes.len()).expect("поле AAD короче 4 ГиБ");
        self.0.extend_from_slice(&len.to_le_bytes());
        self.0.extend_from_slice(bytes);
        self
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn aad() -> Aad {
        Aad::new("test").part(b"field")
    }

    #[test]
    fn roundtrip() {
        let key = Key::random();
        let sealed = seal(&key, &aad(), b"secret");
        assert_eq!(sealed.len(), NONCE_LEN + 6 + TAG_LEN);
        assert_eq!(open(&key, &aad(), &sealed).unwrap().as_slice(), b"secret");
    }

    #[test]
    fn same_plaintext_gives_different_ciphertexts() {
        let key = Key::random();
        assert_ne!(seal(&key, &aad(), b"x"), seal(&key, &aad(), b"x"));
    }

    #[test]
    fn wrong_key_fails() {
        let sealed = seal(&Key::random(), &aad(), b"secret");
        assert_eq!(open(&Key::random(), &aad(), &sealed), Err(AuthError));
    }

    #[test]
    fn wrong_aad_fails() {
        let key = Key::random();
        let sealed = seal(&key, &aad(), b"secret");
        let other = Aad::new("test").part(b"other");
        assert_eq!(open(&key, &other, &sealed), Err(AuthError));
    }

    #[test]
    fn any_flipped_bit_fails() {
        let key = Key::random();
        let sealed = seal(&key, &aad(), b"secret");
        for i in 0..sealed.len() {
            let mut tampered = sealed.clone();
            tampered[i] ^= 0x01;
            assert_eq!(open(&key, &aad(), &tampered), Err(AuthError), "байт {i}");
        }
    }

    #[test]
    fn truncated_fails() {
        let key = Key::random();
        let sealed = seal(&key, &aad(), b"secret");
        for len in 0..sealed.len() {
            assert_eq!(open(&key, &aad(), &sealed[..len]), Err(AuthError));
        }
    }

    #[test]
    fn aad_parts_are_unambiguous() {
        // Без длин эти наборы дали бы одинаковые байты.
        let a = Aad::new("x").part(b"ab").part(b"c");
        let b = Aad::new("x").part(b"a").part(b"bc");
        let c = Aad::new("xa").part(b"bc");
        assert_ne!(a, b);
        assert_ne!(b, c);
    }

    #[test]
    fn random_below_stays_in_range() {
        for n in [1, 2, 3, 7, 62, 1000] {
            for _ in 0..200 {
                assert!(random_below(n) < n);
            }
        }
    }

    #[test]
    fn key_debug_hides_bytes() {
        assert_eq!(format!("{:?}", Key::random()), "Key(..)");
    }

    proptest! {
        #[test]
        fn roundtrip_any(
            plaintext in proptest::collection::vec(any::<u8>(), 0..512),
            context in "[a-z]{0,8}",
            part in proptest::collection::vec(any::<u8>(), 0..32),
        ) {
            let key = Key::random();
            let aad = Aad::new(&context).part(&part);
            let sealed = seal(&key, &aad, &plaintext);
            let opened = open(&key, &aad, &sealed).unwrap();
            prop_assert_eq!(opened.as_slice(), plaintext.as_slice());
        }
    }
}
