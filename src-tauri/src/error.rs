//! Ошибки команд IPC. Интерфейс получает `{ code, message }`: по коду выбирает текст
//! для пользователя, `message` — запасной вариант и подробности для журнала.

use std::fmt;

use serde::Serialize;
use xonkypass_core::Error as CoreError;

#[derive(Debug, Serialize)]
pub struct CommandError {
    code: &'static str,
    message: String,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Команде нужно открытое хранилище, а оно заблокировано.
    pub fn locked() -> Self {
        Self::new("locked", "хранилище заблокировано")
    }

    /// Копировать нечего: поле пустое или у записи такого поля нет.
    pub fn empty_field() -> Self {
        Self::new("empty_field", "поле пустое")
    }

    pub fn clipboard(err: impl fmt::Display) -> Self {
        tracing::warn!(%err, "ошибка буфера обмена");
        Self::new("clipboard", format!("буфер обмена недоступен: {err}"))
    }

    /// Неожиданная ошибка самого приложения (не данных и не пользователя).
    pub fn internal(err: impl fmt::Display) -> Self {
        tracing::error!(%err, "внутренняя ошибка");
        Self::new("internal", err.to_string())
    }

    #[cfg(test)]
    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl From<CoreError> for CommandError {
    fn from(err: CoreError) -> Self {
        let code = err.code();
        match err {
            // Сбои файла и БД важны для разбора проблем; ошибки ввода — нет.
            CoreError::Db(_) | CoreError::Io(_) | CoreError::Kdf(_) | CoreError::Corrupted(_) => {
                tracing::error!(code, %err, "ошибка хранилища");
            }
            _ => tracing::debug!(code, "команда отклонена"),
        }
        Self::new(code, err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_errors_keep_their_codes() {
        assert_eq!(
            CommandError::from(CoreError::WrongPassword).code(),
            "wrong_password"
        );
        assert_eq!(
            CommandError::from(CoreError::ItemNotFound).code(),
            "item_not_found"
        );
    }

    #[test]
    fn serializes_as_code_and_message() {
        let json = serde_json::to_value(CommandError::locked()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "code": "locked", "message": "хранилище заблокировано" })
        );
    }
}
