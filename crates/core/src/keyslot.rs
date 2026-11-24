//! Слоты ключа хранилища (как в LUKS).
//!
//! Ключ хранилища лежит в нескольких зашифрованных копиях — слотах. Каждый слот
//! открывается своим секретом: мастер-паролем, ключом восстановления, позже —
//! Windows Hello. Смена пароля перешифровывает только свой слот, а не все записи.

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::FORMAT_VERSION;
use crate::crypto::{self, Aad, KEY_LEN, Key};
use crate::encoding::{hex_decode, hex_encode};
use crate::error::{Error, Result};
use crate::kdf::{self, KdfParams};

const SALT_LEN: usize = 16;
const ARGON2_VERSION: u32 = 0x13;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlotKind {
    Password,
    Recovery,
}

impl SlotKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            SlotKind::Password => "password",
            SlotKind::Recovery => "recovery",
        }
    }

    fn wrong_secret(self) -> Error {
        match self {
            SlotKind::Password => Error::WrongPassword,
            SlotKind::Recovery => Error::WrongRecoveryKey,
        }
    }
}

/// Слот в том виде, в каком он лежит в таблице `key_slots`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Slot {
    pub kind: SlotKind,
    /// JSON с алгоритмом, параметрами и солью ([`SlotParams`]).
    pub params: String,
    /// `nonce ‖ шифр(ключ хранилища) ‖ тег`.
    pub wrapped_key: Vec<u8>,
}

/// Параметры слота. Поле `kdf` выбирает вариант, чтобы позже добавить слот Windows Hello.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kdf")]
enum SlotParams {
    #[serde(rename = "argon2id")]
    Argon2id {
        version: u32,
        m_kib: u32,
        t: u32,
        p: u32,
        /// Соль в hex.
        salt: String,
    },
}

impl SlotParams {
    fn parse(json: &str) -> Result<Self> {
        serde_json::from_str(json)
            .map_err(|err| Error::Corrupted(format!("параметры слота ключа: {err}")))
    }
}

/// Создаёт слот: выводит ключ слота из `secret` и шифрует им ключ хранилища.
pub(crate) fn seal(
    kind: SlotKind,
    secret: &[u8],
    params: KdfParams,
    vault_key: &Key,
    vault_id: &[u8],
) -> Result<Slot> {
    let mut salt = [0u8; SALT_LEN];
    crypto::fill_random(&mut salt);
    let slot_key = kdf::derive_key(secret, &salt, params)?;

    let stored = SlotParams::Argon2id {
        version: ARGON2_VERSION,
        m_kib: params.m_kib(),
        t: params.t(),
        p: params.p(),
        salt: hex_encode(&salt),
    };
    let wrapped_key = crypto::seal(&slot_key, &aad(kind, vault_id), vault_key.bytes());
    Ok(Slot {
        kind,
        params: serde_json::to_string(&stored).expect("параметры слота сериализуются"),
        wrapped_key,
    })
}

/// Открывает слот и возвращает ключ хранилища.
///
/// Неверный секрет и испорченный слот неотличимы: оба дают [`Error::WrongPassword`]
/// или [`Error::WrongRecoveryKey`].
pub(crate) fn open(slot: &Slot, secret: &[u8], vault_id: &[u8]) -> Result<Key> {
    let SlotParams::Argon2id {
        version,
        m_kib,
        t,
        p,
        salt,
    } = SlotParams::parse(&slot.params)?;
    if version != ARGON2_VERSION {
        return Err(Error::Corrupted(format!(
            "неизвестная версия Argon2: {version:#x}"
        )));
    }
    let salt = hex_decode(&salt)
        .filter(|s| s.len() == SALT_LEN)
        .ok_or_else(|| Error::Corrupted("соль слота ключа".into()))?;
    let slot_key = kdf::derive_key(secret, &salt, KdfParams::new_unchecked(m_kib, t, p))?;

    let plain = crypto::open(&slot_key, &aad(slot.kind, vault_id), &slot.wrapped_key)
        .map_err(|_| slot.kind.wrong_secret())?;
    if plain.len() != KEY_LEN {
        return Err(Error::Corrupted("длина ключа хранилища".into()));
    }
    // Копируем сразу в затираемый буфер, без промежуточного массива на стеке.
    let mut bytes = Zeroizing::new([0u8; KEY_LEN]);
    bytes.copy_from_slice(&plain);
    Ok(Key::from_bytes(bytes))
}

/// Параметры Argon2 слота (для настроек и проверки).
pub(crate) fn kdf_params(slot: &Slot) -> Result<KdfParams> {
    let SlotParams::Argon2id { m_kib, t, p, .. } = SlotParams::parse(&slot.params)?;
    Ok(KdfParams::new_unchecked(m_kib, t, p))
}

fn aad(kind: SlotKind, vault_id: &[u8]) -> Aad {
    Aad::new("slot")
        .part(vault_id)
        .part(kind.as_str().as_bytes())
        .part(&FORMAT_VERSION.to_le_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kdf::tests::FAST;

    const VAULT_ID: [u8; 16] = [7; 16];

    fn sealed(kind: SlotKind, secret: &[u8], key: &Key) -> Slot {
        seal(kind, secret, FAST, key, &VAULT_ID).unwrap()
    }

    fn same_key(a: &Key, b: &Key) -> bool {
        a.bytes() == b.bytes()
    }

    #[test]
    fn roundtrip() {
        let key = Key::random();
        let slot = sealed(SlotKind::Password, b"correct horse", &key);
        let opened = open(&slot, b"correct horse", &VAULT_ID).unwrap();
        assert!(same_key(&key, &opened));
        assert_eq!(kdf_params(&slot).unwrap(), FAST);
    }

    #[test]
    fn params_json_shape() {
        let slot = sealed(SlotKind::Password, b"pw", &Key::random());
        let json: serde_json::Value = serde_json::from_str(&slot.params).unwrap();
        assert_eq!(json["kdf"], "argon2id");
        assert_eq!(json["version"], 19);
        assert_eq!(json["m_kib"], 64);
        assert_eq!(json["salt"].as_str().unwrap().len(), SALT_LEN * 2);
    }

    #[test]
    fn wrong_secret() {
        let key = Key::random();
        let slot = sealed(SlotKind::Password, b"pw", &key);
        assert!(matches!(
            open(&slot, b"Pw", &VAULT_ID),
            Err(Error::WrongPassword)
        ));
        let slot = sealed(SlotKind::Recovery, b"rk", &key);
        assert!(matches!(
            open(&slot, b"rK", &VAULT_ID),
            Err(Error::WrongRecoveryKey)
        ));
    }

    #[test]
    fn bound_to_vault_and_kind() {
        let key = Key::random();
        let slot = sealed(SlotKind::Password, b"pw", &key);
        assert!(open(&slot, b"pw", &[8; 16]).is_err());
        let moved = Slot {
            kind: SlotKind::Recovery,
            ..slot
        };
        assert!(open(&moved, b"pw", &VAULT_ID).is_err());
    }

    #[test]
    fn salt_is_fresh() {
        let key = Key::random();
        let a = sealed(SlotKind::Password, b"pw", &key);
        let b = sealed(SlotKind::Password, b"pw", &key);
        assert_ne!(a.params, b.params);
    }

    #[test]
    fn tampered_params() {
        let key = Key::random();
        let slot = sealed(SlotKind::Password, b"pw", &key);
        for params in [
            String::from("not json"),
            String::from(r#"{"kdf":"scrypt"}"#),
            slot.params.replace("\"version\":19", "\"version\":16"),
            slot.params.replace("\"m_kib\":64", "\"m_kib\":4194304"),
        ] {
            let tampered = Slot {
                params,
                ..slot.clone()
            };
            assert!(
                matches!(open(&tampered, b"pw", &VAULT_ID), Err(Error::Corrupted(_))),
                "{}",
                tampered.params
            );
        }
        // Изменённая соль даёт другой ключ слота — то же, что неверный пароль.
        let SlotParams::Argon2id { salt, .. } = SlotParams::parse(&slot.params).unwrap();
        let tampered = Slot {
            params: slot.params.replace(&salt, &"00".repeat(SALT_LEN)),
            ..slot
        };
        assert!(matches!(
            open(&tampered, b"pw", &VAULT_ID),
            Err(Error::WrongPassword)
        ));
    }
}
