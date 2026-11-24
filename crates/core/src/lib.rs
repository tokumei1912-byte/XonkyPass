//! Ядро XonkyPass: шифрование, хранилище, поиск.
//!
//! Крейт не зависит от Tauri и Windows, поэтому тестируется на любой платформе.
//! Наполняется на этапе 1 (см. `docs/PLAN.md`).

#![forbid(unsafe_code)]

/// Версия формата файла хранилища.
pub const FORMAT_VERSION: u32 = 1;

/// Версия ядра.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver() {
        assert_eq!(version().split('.').count(), 3);
    }
}
