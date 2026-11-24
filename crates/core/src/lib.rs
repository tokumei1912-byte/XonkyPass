//! Ядро XonkyPass: шифрование, хранилище, генератор паролей.
//!
//! Крейт не зависит от Tauri и Windows, поэтому тестируется на любой платформе.
//! Формат файла хранилища описан в `docs/FORMAT.md`.
//!
//! Главный тип — [`Vault`], разблокированное хранилище:
//!
//! ```no_run
//! use std::path::Path;
//! use xonkypass_core::{ItemMeta, ItemSecret, KdfParams, LoginMeta, Scope, Vault, kdf};
//!
//! # fn main() -> xonkypass_core::Result<()> {
//! let path = Path::new("vault.xpv");
//! let params = KdfParams::calibrate(kdf::UNLOCK_TARGET);
//! let (mut vault, recovery_key) = Vault::create(path, "мастер-пароль", params)?;
//! println!("Сохраните ключ восстановления: {}", recovery_key.to_display().as_str());
//!
//! let meta = ItemMeta::Login(LoginMeta {
//!     title: "GitHub".into(),
//!     username: "user@example.com".into(),
//!     urls: vec!["github.com".into()],
//!     ..Default::default()
//! });
//! let id = vault.create_item(meta, ItemSecret::with_password("s3cr3t"))?;
//! vault.lock();
//!
//! let vault = Vault::unlock(path, "мастер-пароль")?;
//! assert_eq!(vault.items(Scope::Active)?.items.len(), 1);
//! assert_eq!(vault.reveal(id)?.password, "s3cr3t");
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]

mod crypto;
mod encoding;
mod error;
pub mod generator;
pub mod kdf;
mod keyslot;
mod model;
mod recovery;
pub mod search;
mod store;
mod vault;

pub use error::{Error, Invalid, Result};
pub use kdf::KdfParams;
pub use model::{
    CustomField, ItemId, ItemKind, ItemMeta, ItemSecret, LoginMeta, NoteMeta, limits, normalize_tag,
};
pub use recovery::RecoveryKey;
pub use search::{SearchOptions, SearchResult, SortOrder, TagCount};
pub use vault::{HISTORY_LIMIT, HistoryEntry, ItemList, ItemSummary, Scope, UnlockMethod, Vault};

/// Версия формата хранилища. Входит в связанные данные каждого шифртекста.
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
