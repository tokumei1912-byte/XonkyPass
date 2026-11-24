//! Тесты хранилища целиком: от файла на диске до записей.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tempfile::TempDir;

use super::*;
use crate::kdf::tests::FAST;
use crate::model::{CustomField, LoginMeta, NoteMeta};

const PASSWORD: &str = "correct horse battery staple";

struct Fixture {
    _dir: TempDir,
    path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.xpv");
        Self { _dir: dir, path }
    }

    fn create(&self) -> (Vault, RecoveryKey) {
        Vault::create(&self.path, PASSWORD, FAST).unwrap()
    }

    fn unlock(&self) -> Vault {
        Vault::unlock(&self.path, PASSWORD).unwrap()
    }

    /// Прямой доступ к файлу в обход хранилища — как у злоумышленника.
    fn raw(&self) -> Connection {
        Connection::open(&self.path).unwrap()
    }
}

fn login(title: &str) -> ItemMeta {
    ItemMeta::Login(LoginMeta {
        title: title.into(),
        username: format!("{title}@example.com"),
        urls: vec![format!("{}.com", title.to_lowercase())],
        ..Default::default()
    })
}

fn note(title: &str, body: &str) -> ItemMeta {
    ItemMeta::Note(NoteMeta {
        title: title.into(),
        body: body.into(),
        ..Default::default()
    })
}

fn secret(password: &str) -> ItemSecret {
    ItemSecret::with_password(password)
}

fn blob(conn: &Connection, sql: &str, id: ItemId) -> Vec<u8> {
    conn.query_row(sql, [id.as_bytes()], |r| r.get(0)).unwrap()
}

fn set_blob(conn: &Connection, sql: &str, id: ItemId, value: &[u8]) {
    assert_eq!(
        conn.execute(sql, rusqlite::params![id.as_bytes(), value])
            .unwrap(),
        1
    );
}

// --- Создание и разблокировка ---

#[test]
fn create_and_unlock() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    assert_eq!(vault.unlocked_with(), UnlockMethod::Password);
    let id = vault
        .create_item(login("GitHub"), secret("hunter2"))
        .unwrap();
    vault.lock();

    let vault = fx.unlock();
    assert_eq!(vault.item(id).unwrap().meta.title(), "GitHub");
    assert_eq!(vault.reveal(id).unwrap().password, "hunter2");
    assert_eq!(vault.kdf_params().unwrap(), FAST);
}

#[test]
fn create_refuses_existing_and_empty_password() {
    let fx = Fixture::new();
    std::fs::write(&fx.path, b"precious").unwrap();
    assert!(matches!(
        Vault::create(&fx.path, PASSWORD, FAST),
        Err(Error::AlreadyExists)
    ));
    assert_eq!(std::fs::read(&fx.path).unwrap(), b"precious");

    let fx = Fixture::new();
    assert!(matches!(
        Vault::create(&fx.path, "", FAST),
        Err(Error::Invalid(Invalid::EmptyPassword))
    ));
    assert!(!fx.path.exists());
}

#[test]
fn wrong_password() {
    let fx = Fixture::new();
    fx.create();
    for wrong in [
        "",
        "Correct horse battery staple",
        "correct horse battery staple ",
    ] {
        assert!(matches!(
            Vault::unlock(&fx.path, wrong),
            Err(Error::WrongPassword)
        ));
    }
}

#[test]
fn unlock_missing_vault() {
    let fx = Fixture::new();
    assert!(!Vault::exists(&fx.path));
    assert!(matches!(
        Vault::unlock(&fx.path, PASSWORD),
        Err(Error::VaultNotFound)
    ));
    fx.create();
    assert!(Vault::exists(&fx.path));
}

#[test]
fn recovery_key_unlocks_and_resets_password() {
    let fx = Fixture::new();
    let (mut vault, recovery) = fx.create();
    let id = vault.create_item(login("Bank"), secret("1234")).unwrap();
    vault.lock();

    let text = recovery.to_display();
    let parsed = RecoveryKey::parse(&text.to_lowercase()).unwrap();
    let mut vault = Vault::unlock_with_recovery_key(&fx.path, &parsed).unwrap();
    assert_eq!(vault.unlocked_with(), UnlockMethod::RecoveryKey);
    assert_eq!(vault.reveal(id).unwrap().password, "1234");

    vault.reset_password("new password", FAST).unwrap();
    vault.lock();
    assert!(matches!(
        Vault::unlock(&fx.path, PASSWORD),
        Err(Error::WrongPassword)
    ));
    let vault = Vault::unlock(&fx.path, "new password").unwrap();
    assert_eq!(vault.reveal(id).unwrap().password, "1234");
}

#[test]
fn wrong_recovery_key() {
    let fx = Fixture::new();
    fx.create();
    let other = RecoveryKey::generate();
    assert!(matches!(
        Vault::unlock_with_recovery_key(&fx.path, &other),
        Err(Error::WrongRecoveryKey)
    ));
}

#[test]
fn reset_password_requires_recovery_unlock() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    assert!(matches!(
        vault.reset_password("new", FAST),
        Err(Error::NotAllowed(_))
    ));
}

#[test]
fn change_password() {
    let fx = Fixture::new();
    let (mut vault, recovery) = fx.create();
    let id = vault.create_item(login("Mail"), secret("pw")).unwrap();

    assert!(matches!(
        vault.change_password("wrong", "new", FAST),
        Err(Error::WrongPassword)
    ));
    assert!(matches!(
        vault.change_password(PASSWORD, "", FAST),
        Err(Error::Invalid(Invalid::EmptyPassword))
    ));
    let new_params = KdfParams::new_unchecked(128, 1, 1);
    vault.change_password(PASSWORD, "new", new_params).unwrap();
    assert_eq!(vault.kdf_params().unwrap(), new_params);
    vault.verify_password("new").unwrap();
    vault.lock();

    assert!(Vault::unlock(&fx.path, PASSWORD).is_err());
    let vault = Vault::unlock(&fx.path, "new").unwrap();
    assert_eq!(vault.reveal(id).unwrap().password, "pw");
    vault.lock();
    // Ключ восстановления продолжает работать: его слот не трогали.
    Vault::unlock_with_recovery_key(&fx.path, &recovery).unwrap();
}

#[test]
fn regenerate_recovery_key() {
    let fx = Fixture::new();
    let (mut vault, old) = fx.create();
    let new = vault.regenerate_recovery_key().unwrap();
    vault.lock();
    assert!(matches!(
        Vault::unlock_with_recovery_key(&fx.path, &old),
        Err(Error::WrongRecoveryKey)
    ));
    Vault::unlock_with_recovery_key(&fx.path, &new).unwrap();
}

#[test]
fn vault_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Vault>();
}

// --- Записи ---

#[test]
fn item_crud() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let a = vault.create_item(login("A"), secret("pa")).unwrap();
    let b = vault
        .create_item(note("B", "текст"), ItemSecret::default())
        .unwrap();

    let list = vault.items(Scope::Active).unwrap();
    assert!(list.damaged.is_empty());
    let ids: Vec<ItemId> = list.items.iter().map(|i| i.id).collect();
    assert_eq!(ids, [a, b]);
    assert_eq!(list.items[0].revision, 1);

    vault.update_item(a, login("A2"), None).unwrap();
    let item = vault.item(a).unwrap();
    assert_eq!(item.meta.title(), "A2");
    assert_eq!(item.revision, 2);
    assert!(item.updated_at >= item.created_at);
    assert_eq!(vault.reveal(a).unwrap().password, "pa");

    vault
        .update_item(a, login("A2"), Some(secret("new")))
        .unwrap();
    assert_eq!(vault.reveal(a).unwrap().password, "new");

    assert!(matches!(
        vault.item(ItemId::generate()),
        Err(Error::ItemNotFound)
    ));
}

#[test]
fn update_without_changes_is_noop() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let id = vault.create_item(login("A"), secret("p")).unwrap();
    vault.update_item(id, login("A"), None).unwrap();
    vault
        .update_item(id, login("A"), Some(secret("p")))
        .unwrap();
    assert_eq!(vault.item(id).unwrap().revision, 1);
    assert!(vault.history(id).unwrap().is_empty());
}

#[test]
fn kind_cannot_change() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let id = vault.create_item(login("A"), secret("p")).unwrap();
    assert!(matches!(
        vault.update_item(id, note("A", "x"), None),
        Err(Error::Invalid(Invalid::KindChanged))
    ));
}

#[test]
fn invalid_items_are_rejected() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    assert!(matches!(
        vault.create_item(ItemMeta::Login(LoginMeta::default()), secret("p")),
        Err(Error::Invalid(Invalid::EmptyTitle))
    ));
    assert!(matches!(
        vault.create_item(note("", ""), ItemSecret::default()),
        Err(Error::Invalid(Invalid::EmptyNote))
    ));
    assert!(vault.items(Scope::Active).unwrap().items.is_empty());
}

#[test]
fn has_totp_follows_secret() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let mut meta = login("A");
    if let ItemMeta::Login(m) = &mut meta {
        m.has_totp = true; // интерфейс не может это навязать
    }
    let id = vault.create_item(meta, secret("p")).unwrap();
    let has_totp = |vault: &Vault| match vault.item(id).unwrap().meta {
        ItemMeta::Login(m) => m.has_totp,
        ItemMeta::Note(_) => unreachable!(),
    };
    assert!(!has_totp(&vault));

    let mut with_totp = secret("p");
    with_totp.totp = "otpauth://totp/A?secret=JBSWY3DPEHPK3PXP".into();
    vault.update_item(id, login("A"), Some(with_totp)).unwrap();
    assert!(has_totp(&vault));
    // Изменение метаданных без секрета сохраняет признак.
    vault.update_item(id, login("A"), None).unwrap();
    assert!(has_totp(&vault));
}

#[test]
fn custom_fields_roundtrip() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let mut s = secret("p");
    s.fields = vec![CustomField {
        name: "PIN".into(),
        value: "0000".into(),
        hidden: true,
    }];
    s.notes = "заметка".into();
    let id = vault.create_item(login("Card"), s.clone()).unwrap();
    assert_eq!(vault.reveal(id).unwrap(), s);
}

#[test]
fn usage_is_recorded() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let id = vault.create_item(login("A"), secret("p")).unwrap();
    vault.record_use(id).unwrap();
    vault.record_use(id).unwrap();
    let item = vault.item(id).unwrap();
    assert_eq!(item.use_count, 2);
    assert!(item.last_used_at.is_some());
    assert!(matches!(
        vault.record_use(ItemId::generate()),
        Err(Error::ItemNotFound)
    ));
}

// --- Корзина ---

#[test]
fn trash_restore_purge() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let a = vault.create_item(login("A"), secret("p")).unwrap();
    let b = vault.create_item(login("B"), secret("p")).unwrap();

    assert!(matches!(vault.purge(a), Err(Error::NotInTrash)));

    vault.trash(a).unwrap();
    vault.trash(a).unwrap();
    let active: Vec<ItemId> = vault
        .items(Scope::Active)
        .unwrap()
        .items
        .iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(active, [b]);
    let trash = vault.items(Scope::Trash).unwrap().items;
    assert_eq!(trash.len(), 1);
    assert!(trash[0].deleted_at.is_some());

    vault.restore(a).unwrap();
    assert!(vault.item(a).unwrap().deleted_at.is_none());
    assert_eq!(vault.items(Scope::Active).unwrap().items.len(), 2);

    vault.trash(a).unwrap();
    vault.purge(a).unwrap();
    assert!(matches!(vault.item(a), Err(Error::ItemNotFound)));

    vault.trash(b).unwrap();
    assert_eq!(vault.empty_trash().unwrap(), 1);
    assert!(vault.items(Scope::Active).unwrap().items.is_empty());
    assert!(vault.items(Scope::Trash).unwrap().items.is_empty());
}

#[test]
fn purge_removes_ciphertext_from_file() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let id = vault.create_item(login("A"), secret("p")).unwrap();
    let meta_ct = blob(&fx.raw(), "SELECT meta FROM items WHERE id = ?1", id);
    vault.update_item(id, login("A2"), None).unwrap();
    vault.trash(id).unwrap();
    vault.purge(id).unwrap();
    vault.lock();

    // secure_delete + WAL-checkpoint при закрытии: старого шифртекста в файле нет.
    let file = std::fs::read(&fx.path).unwrap();
    let needle = &meta_ct[..32];
    assert!(!file.windows(needle.len()).any(|w| w == needle));
}

// --- История ---

#[test]
fn history_keeps_previous_versions() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let id = vault.create_item(login("A"), secret("v1")).unwrap();
    let created = vault.item(id).unwrap().updated_at;
    vault
        .update_item(id, login("A"), Some(secret("v2")))
        .unwrap();
    vault
        .update_item(id, login("A"), Some(secret("v3")))
        .unwrap();

    let history = vault.history(id).unwrap();
    let passwords: Vec<&str> = history.iter().map(|h| h.secret.password.as_str()).collect();
    assert_eq!(passwords, ["v2", "v1"]);
    assert_eq!(history[1].updated_at, created);
    assert!(history[0].replaced_at >= history[1].replaced_at);
}

#[test]
fn history_ignores_tags_and_favorite() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let id = vault.create_item(login("A"), secret("p")).unwrap();
    let mut meta = login("A");
    if let ItemMeta::Login(m) = &mut meta {
        m.favorite = true;
        m.tags = vec!["работа".into()];
    }
    vault.update_item(id, meta, None).unwrap();
    assert_eq!(vault.item(id).unwrap().revision, 2);
    assert!(vault.history(id).unwrap().is_empty());

    vault.update_item(id, login("Renamed"), None).unwrap();
    assert_eq!(vault.history(id).unwrap().len(), 1);
}

#[test]
fn history_is_limited() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let id = vault.create_item(login("A"), secret("0")).unwrap();
    for i in 1..=HISTORY_LIMIT + 5 {
        vault
            .update_item(id, login("A"), Some(secret(&i.to_string())))
            .unwrap();
    }
    let history = vault.history(id).unwrap();
    assert_eq!(history.len(), HISTORY_LIMIT);
    assert_eq!(history[0].secret.password, (HISTORY_LIMIT + 4).to_string());
}

// --- Подмена и повреждение данных ---

#[test]
fn swapped_meta_between_items_is_detected() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let a = vault.create_item(login("A"), secret("pa")).unwrap();
    let b = vault.create_item(login("B"), secret("pb")).unwrap();
    let c = vault.create_item(login("C"), secret("pc")).unwrap();

    let raw = fx.raw();
    let meta_a = blob(&raw, "SELECT meta FROM items WHERE id = ?1", a);
    let meta_b = blob(&raw, "SELECT meta FROM items WHERE id = ?1", b);
    set_blob(&raw, "UPDATE items SET meta = ?2 WHERE id = ?1", a, &meta_b);
    set_blob(&raw, "UPDATE items SET meta = ?2 WHERE id = ?1", b, &meta_a);

    let list = vault.items(Scope::Active).unwrap();
    assert_eq!(list.damaged, [a, b]);
    assert_eq!(list.items.len(), 1);
    assert_eq!(list.items[0].id, c);
    assert!(matches!(vault.item(a), Err(Error::Damaged(id)) if id == a));
}

#[test]
fn swapped_secret_between_items_is_detected() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let a = vault.create_item(login("A"), secret("pa")).unwrap();
    let b = vault.create_item(login("B"), secret("pb")).unwrap();

    let raw = fx.raw();
    let secret_b = blob(&raw, "SELECT secret FROM items WHERE id = ?1", b);
    set_blob(
        &raw,
        "UPDATE items SET secret = ?2 WHERE id = ?1",
        a,
        &secret_b,
    );

    assert!(matches!(vault.reveal(a), Err(Error::Damaged(_))));
    assert_eq!(vault.reveal(b).unwrap().password, "pb");
}

#[test]
fn meta_and_secret_fields_cannot_be_swapped() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let a = vault.create_item(login("A"), secret("pa")).unwrap();

    let raw = fx.raw();
    let meta = blob(&raw, "SELECT meta FROM items WHERE id = ?1", a);
    let secret = blob(&raw, "SELECT secret FROM items WHERE id = ?1", a);
    set_blob(&raw, "UPDATE items SET meta = ?2 WHERE id = ?1", a, &secret);
    set_blob(&raw, "UPDATE items SET secret = ?2 WHERE id = ?1", a, &meta);

    assert_eq!(vault.items(Scope::Active).unwrap().damaged, [a]);
    assert!(matches!(vault.reveal(a), Err(Error::Damaged(_))));
}

#[test]
fn renamed_item_id_is_detected() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let a = vault.create_item(login("A"), secret("pa")).unwrap();
    let new_id = ItemId::generate();
    set_blob(
        &fx.raw(),
        "UPDATE items SET id = ?2 WHERE id = ?1",
        a,
        new_id.as_bytes(),
    );
    assert_eq!(vault.items(Scope::Active).unwrap().damaged, [new_id]);
}

#[test]
fn item_from_another_vault_is_detected() {
    // Даже с тем же паролем: у каждого хранилища свой ключ и свой vault_id.
    let first = Fixture::new();
    let (mut vault1, _) = first.create();
    let id = vault1.create_item(login("A"), secret("pa")).unwrap();
    vault1.lock();

    let second = Fixture::new();
    let (vault2, _) = second.create();
    let raw = second.raw();
    raw.execute(
        "ATTACH DATABASE ?1 AS other",
        [first.path.to_str().unwrap()],
    )
    .unwrap();
    raw.execute("INSERT INTO items SELECT * FROM other.items", [])
        .unwrap();

    assert_eq!(vault2.items(Scope::Active).unwrap().damaged, [id]);
}

#[test]
fn password_slot_from_another_vault_is_rejected() {
    // Пароль от одного хранилища не должен открывать другое, даже если
    // подложить в него чужой слот.
    let first = Fixture::new();
    first.create();
    let second = Fixture::new();
    let (vault2, _) = second.create();
    vault2.lock();

    let raw = second.raw();
    raw.execute(
        "ATTACH DATABASE ?1 AS other",
        [first.path.to_str().unwrap()],
    )
    .unwrap();
    raw.execute(
        "UPDATE key_slots SET (params, wrapped_key) =
           (SELECT params, wrapped_key FROM other.key_slots WHERE kind = 'password')
         WHERE kind = 'password'",
        [],
    )
    .unwrap();
    drop(raw);

    assert!(matches!(
        Vault::unlock(&second.path, PASSWORD),
        Err(Error::WrongPassword)
    ));
}

#[test]
fn flipped_bit_in_item_is_detected() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let a = vault.create_item(login("A"), secret("pa")).unwrap();
    let raw = fx.raw();
    let mut secret = blob(&raw, "SELECT secret FROM items WHERE id = ?1", a);
    let last = secret.len() - 1;
    secret[last] ^= 0x80;
    set_blob(
        &raw,
        "UPDATE items SET secret = ?2 WHERE id = ?1",
        a,
        &secret,
    );
    assert!(matches!(vault.reveal(a), Err(Error::Damaged(_))));
    // Метаданные целы — запись видна в списке.
    assert!(vault.items(Scope::Active).unwrap().damaged.is_empty());
}

#[test]
fn swapped_history_is_detected() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let a = vault.create_item(login("A"), secret("a1")).unwrap();
    let b = vault.create_item(login("B"), secret("b1")).unwrap();
    vault
        .update_item(a, login("A"), Some(secret("a2")))
        .unwrap();
    vault
        .update_item(b, login("B"), Some(secret("b2")))
        .unwrap();

    let raw = fx.raw();
    raw.execute(
        "UPDATE item_history SET item_id = ?1 WHERE item_id = ?2",
        [a.as_bytes(), b.as_bytes()],
    )
    .unwrap();
    assert!(matches!(vault.history(a), Err(Error::Damaged(_))));

    // Подмена даты снимка тоже ломает проверку.
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    let a = vault.create_item(login("A"), secret("a1")).unwrap();
    vault
        .update_item(a, login("A"), Some(secret("a2")))
        .unwrap();
    fx.raw()
        .execute("UPDATE item_history SET created_at = created_at + 1", [])
        .unwrap();
    assert!(matches!(vault.history(a), Err(Error::Damaged(_))));
}

#[test]
fn garbage_file_is_not_a_vault() {
    let fx = Fixture::new();
    std::fs::write(
        &fx.path,
        b"definitely not sqlite, just some text".repeat(300),
    )
    .unwrap();
    assert!(matches!(
        Vault::unlock(&fx.path, PASSWORD),
        Err(Error::NotAVault)
    ));
}

fn corrupt(path: &Path, f: impl FnOnce(&mut Vec<u8>)) {
    let mut bytes = std::fs::read(path).unwrap();
    f(&mut bytes);
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn damaged_pages_are_reported() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    for i in 0..20 {
        vault
            .create_item(
                note(&format!("N{i}"), &"x".repeat(2000)),
                ItemSecret::default(),
            )
            .unwrap();
    }
    vault.lock();

    let page_size = 4096;
    corrupt(&fx.path, |bytes| {
        assert!(bytes.len() > 3 * page_size);
        // Вторая страница — корень одной из таблиц.
        bytes[page_size..2 * page_size].fill(0xa5);
    });
    let err = Vault::unlock(&fx.path, PASSWORD).unwrap_err();
    assert!(
        matches!(err, Error::Corrupted(_) | Error::NotAVault),
        "{err:?}"
    );
}

#[test]
fn truncated_file_is_reported() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    for i in 0..20 {
        vault
            .create_item(
                note(&format!("N{i}"), &"x".repeat(2000)),
                ItemSecret::default(),
            )
            .unwrap();
    }
    vault.lock();

    corrupt(&fx.path, |bytes| bytes.truncate(bytes.len() / 2));
    let err = Vault::unlock(&fx.path, PASSWORD).unwrap_err();
    assert!(
        matches!(err, Error::Corrupted(_) | Error::NotAVault),
        "{err:?}"
    );
}

#[test]
fn missing_slot_is_reported() {
    let fx = Fixture::new();
    fx.create().0.lock();
    fx.raw()
        .execute("DELETE FROM key_slots WHERE kind = 'password'", [])
        .unwrap();
    assert!(matches!(
        Vault::unlock(&fx.path, PASSWORD),
        Err(Error::Corrupted(_))
    ));
}

#[test]
fn newer_format_is_rejected() {
    let fx = Fixture::new();
    fx.create().0.lock();
    fx.raw()
        .execute("UPDATE vault SET format_ver = format_ver + 1", [])
        .unwrap();
    assert!(matches!(
        Vault::unlock(&fx.path, PASSWORD),
        Err(Error::UnsupportedVersion)
    ));
}

// --- Что лежит в файле открыто ---

#[test]
fn file_contains_no_plaintext() {
    let fx = Fixture::new();
    let (mut vault, _) = fx.create();
    vault
        .create_item(
            ItemMeta::Login(LoginMeta {
                title: "UniqueTitleMarker".into(),
                username: "unique.user.marker@example.com".into(),
                urls: vec!["unique-site-marker.example".into()],
                tags: vec!["UniqueTagMarker".into()],
                ..Default::default()
            }),
            secret("UniquePasswordMarker"),
        )
        .unwrap();
    vault
        .create_item(
            note("UniqueNoteTitle", "UniqueNoteBodyMarker"),
            ItemSecret::default(),
        )
        .unwrap();
    vault.lock();

    let file = std::fs::read(&fx.path).unwrap();
    for marker in [
        "UniqueTitleMarker",
        "unique.user.marker",
        "unique-site-marker",
        "UniqueTagMarker",
        "UniquePasswordMarker",
        "UniqueNoteTitle",
        "UniqueNoteBodyMarker",
        PASSWORD,
    ] {
        assert!(
            !file.windows(marker.len()).any(|w| w == marker.as_bytes()),
            "в файле открыто лежит {marker}"
        );
    }
}
