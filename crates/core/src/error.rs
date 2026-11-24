//! Ошибки ядра.
//!
//! Интерфейс получает ошибку как `{ code, message }`: `code` — стабильный
//! идентификатор из [`Error::code`], по нему берётся текст для пользователя.

use crate::model::ItemId;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("хранилище не найдено")]
    VaultNotFound,

    #[error("хранилище уже существует")]
    AlreadyExists,

    #[error("файл не является хранилищем XonkyPass")]
    NotAVault,

    #[error("хранилище создано более новой версией XonkyPass")]
    UnsupportedVersion,

    #[error("хранилище повреждено: {0}")]
    Corrupted(String),

    /// Неверный пароль и повреждённый слот ключа неотличимы: в обоих случаях
    /// не сходится проверка подлинности шифртекста.
    #[error("неверный мастер-пароль")]
    WrongPassword,

    #[error("неверный ключ восстановления")]
    WrongRecoveryKey,

    /// Ключ восстановления не прошёл проверку формата или контрольной суммы:
    /// скорее всего, опечатка.
    #[error("ключ восстановления введён с ошибкой")]
    InvalidRecoveryKey,

    #[error("запись не найдена")]
    ItemNotFound,

    #[error("запись {0} повреждена или подменена")]
    Damaged(ItemId),

    #[error("запись не в корзине")]
    NotInTrash,

    #[error("действие недоступно: {0}")]
    NotAllowed(&'static str),

    #[error(transparent)]
    Invalid(#[from] Invalid),

    #[error("ошибка Argon2: {0}")]
    Kdf(argon2::Error),

    #[error("ошибка базы данных: {0}")]
    Db(rusqlite::Error),

    #[error("ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Стабильный код ошибки для интерфейса.
    pub fn code(&self) -> &'static str {
        match self {
            Error::VaultNotFound => "vault_not_found",
            Error::AlreadyExists => "already_exists",
            Error::NotAVault => "not_a_vault",
            Error::UnsupportedVersion => "unsupported_version",
            Error::Corrupted(_) => "corrupted",
            Error::WrongPassword => "wrong_password",
            Error::WrongRecoveryKey => "wrong_recovery_key",
            Error::InvalidRecoveryKey => "invalid_recovery_key",
            Error::ItemNotFound => "item_not_found",
            Error::Damaged(_) => "damaged",
            Error::NotInTrash => "not_in_trash",
            Error::NotAllowed(_) => "not_allowed",
            Error::Invalid(_) => "invalid",
            Error::Kdf(_) => "kdf",
            Error::Db(_) => "db",
            Error::Io(_) => "io",
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        use rusqlite::ErrorCode;
        match err.sqlite_error_code() {
            Some(ErrorCode::NotADatabase) => Error::NotAVault,
            Some(ErrorCode::DatabaseCorrupt) => Error::Corrupted(err.to_string()),
            _ => Error::Db(err),
        }
    }
}

/// Неверные данные от пользователя или интерфейса.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Invalid {
    #[error("пустой мастер-пароль")]
    EmptyPassword,

    #[error("пустое название")]
    EmptyTitle,

    #[error("пустая заметка")]
    EmptyNote,

    #[error("поле «{field}» длиннее {max} символов")]
    TooLong { field: &'static str, max: usize },

    #[error("в поле «{field}» больше {max} значений")]
    TooMany { field: &'static str, max: usize },

    #[error("у дополнительного поля нет названия")]
    EmptyFieldName,

    #[error("нельзя изменить тип записи")]
    KindChanged,

    #[error("не выбран ни один набор символов")]
    NoCharacterSets,

    #[error("длина должна быть от {min} до {max}")]
    OutOfRange { min: usize, max: usize },
}
