//! Файл хранилища: SQLite, схема, миграции и работа со строками таблиц.
//!
//! Здесь нет шифрования: модуль хранит и отдаёт уже зашифрованные байты.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::config::DbConfig;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, params};

use crate::error::{Error, Result};
use crate::keyslot::{Slot, SlotKind};
use crate::model::ItemId;

/// `PRAGMA application_id` = «XPV1»: отличает хранилище от других файлов SQLite.
pub(crate) const APPLICATION_ID: i32 = 0x5850_5631;

/// Миграции схемы по порядку; номер версии схемы = число применённых миграций
/// (`PRAGMA user_version`).
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql")];

pub(crate) const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Сколько сообщений `quick_check` показывать в ошибке.
const MAX_CHECK_MESSAGES: usize = 5;

/// Создаёт новый файл хранилища с актуальной схемой.
pub(crate) fn create(path: &Path) -> Result<Connection> {
    if path.exists() {
        return Err(Error::AlreadyExists);
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let mut conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    harden(&conn)?;
    conn.pragma_update(None, "application_id", APPLICATION_ID)?;
    configure(&conn)?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// Открывает существующий файл: проверяет, что это хранилище XonkyPass и что оно
/// не повреждено, и при необходимости обновляет схему.
pub(crate) fn open(path: &Path) -> Result<Connection> {
    if !path.is_file() {
        return Err(Error::VaultNotFound);
    }
    let mut conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    harden(&conn)?;
    let app_id: i32 = conn.pragma_query_value(None, "application_id", |r| r.get(0))?;
    if app_id != APPLICATION_ID {
        return Err(Error::NotAVault);
    }
    quick_check(&conn)?;
    configure(&conn)?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// Удаляет файл хранилища и служебные файлы SQLite рядом с ним (после неудачного создания).
pub(crate) fn remove_files(path: &Path) {
    let _ = std::fs::remove_file(path);
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sidecar = PathBuf::from(path).into_os_string();
        sidecar.push(suffix);
        let _ = std::fs::remove_file(sidecar);
    }
}

/// Защита от подложенного файла: схема не может вызывать функции с побочными
/// эффектами, а служебные таблицы нельзя менять напрямую.
fn harden(conn: &Connection) -> Result<()> {
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    Ok(())
}

fn configure(conn: &Connection) -> Result<()> {
    // WAL: запись не блокирует чтение, при сбое файл остаётся целым.
    // На файловых системах без WAL SQLite останется в обычном режиме — это не ошибка.
    let _mode: String = conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
    // FULL: сохранённый пароль не теряется даже при отключении питания.
    conn.pragma_update(None, "synchronous", "FULL")?;
    // Удалённые данные затираются нулями, а не остаются в свободных страницах.
    conn.pragma_update(None, "secure_delete", "ON")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

fn quick_check(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA quick_check")?;
    let messages = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .take(MAX_CHECK_MESSAGES)
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if messages.len() == 1 && messages[0] == "ok" {
        Ok(())
    } else {
        Err(Error::Corrupted(messages.join("; ")))
    }
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    let applied = usize::try_from(version)
        .map_err(|_| Error::Corrupted(format!("версия схемы {version}")))?;
    for (version, sql) in (1i64..).zip(MIGRATIONS).skip(applied) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }
    Ok(())
}

/// Строка таблицы `vault`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Header {
    pub vault_id: [u8; 16],
    pub format_ver: u32,
    pub created_at: i64,
}

pub(crate) fn insert_header(conn: &Connection, header: &Header) -> Result<()> {
    conn.execute(
        "INSERT INTO vault (id, vault_id, format_ver, created_at) VALUES (1, ?1, ?2, ?3)",
        params![header.vault_id, header.format_ver, header.created_at],
    )?;
    Ok(())
}

pub(crate) fn read_header(conn: &Connection) -> Result<Header> {
    conn.query_row(
        "SELECT vault_id, format_ver, created_at FROM vault WHERE id = 1",
        [],
        |r| {
            Ok(Header {
                vault_id: r.get(0)?,
                format_ver: r.get(1)?,
                created_at: r.get(2)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| Error::Corrupted("нет заголовка хранилища".into()))
}

pub(crate) fn put_slot(conn: &Connection, slot: &Slot) -> Result<()> {
    conn.execute(
        "INSERT INTO key_slots (kind, params, wrapped_key) VALUES (?1, ?2, ?3)
         ON CONFLICT (kind) DO UPDATE SET params = excluded.params, wrapped_key = excluded.wrapped_key",
        params![slot.kind.as_str(), slot.params, slot.wrapped_key],
    )?;
    Ok(())
}

pub(crate) fn get_slot(conn: &Connection, kind: SlotKind) -> Result<Option<Slot>> {
    Ok(conn
        .query_row(
            "SELECT params, wrapped_key FROM key_slots WHERE kind = ?1",
            [kind.as_str()],
            |r| {
                Ok(Slot {
                    kind,
                    params: r.get(0)?,
                    wrapped_key: r.get(1)?,
                })
            },
        )
        .optional()?)
}

/// Строка таблицы `items` вместе со счётчиками использования.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ItemRow {
    pub id: ItemId,
    pub meta: Vec<u8>,
    pub secret: Vec<u8>,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub revision: i64,
    pub use_count: i64,
    pub last_used_at: Option<i64>,
}

const ITEM_SELECT: &str = "SELECT i.id, i.meta, i.secret, i.created_at, i.updated_at, i.deleted_at,
            i.revision, coalesce(u.use_count, 0), u.last_used_at
     FROM items i LEFT JOIN item_usage u ON u.item_id = i.id";

fn item_row(r: &Row<'_>) -> rusqlite::Result<ItemRow> {
    Ok(ItemRow {
        id: ItemId::from_bytes(r.get(0)?),
        meta: r.get(1)?,
        secret: r.get(2)?,
        created_at: r.get(3)?,
        updated_at: r.get(4)?,
        deleted_at: r.get(5)?,
        revision: r.get(6)?,
        use_count: r.get(7)?,
        last_used_at: r.get(8)?,
    })
}

pub(crate) fn insert_item(
    conn: &Connection,
    id: ItemId,
    meta: &[u8],
    secret: &[u8],
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO items (id, meta, secret, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id.as_bytes(), meta, secret, now],
    )?;
    Ok(())
}

pub(crate) fn get_item(conn: &Connection, id: ItemId) -> Result<Option<ItemRow>> {
    let mut stmt = conn.prepare_cached(&format!("{ITEM_SELECT} WHERE i.id = ?1"))?;
    Ok(stmt.query_row([id.as_bytes()], item_row).optional()?)
}

/// Записи из корзины (`trashed`) или вне её, в порядке создания.
pub(crate) fn list_items(conn: &Connection, trashed: bool) -> Result<Vec<ItemRow>> {
    let filter = if trashed {
        "i.deleted_at IS NOT NULL"
    } else {
        "i.deleted_at IS NULL"
    };
    let mut stmt = conn.prepare_cached(&format!("{ITEM_SELECT} WHERE {filter} ORDER BY i.id"))?;
    let rows = stmt
        .query_map([], item_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub(crate) fn update_content(
    conn: &Connection,
    id: ItemId,
    meta: &[u8],
    secret: &[u8],
    now: i64,
) -> Result<()> {
    let changed = conn.execute(
        "UPDATE items SET meta = ?2, secret = ?3, updated_at = ?4, revision = revision + 1
         WHERE id = ?1",
        params![id.as_bytes(), meta, secret, now],
    )?;
    expect_one(changed)
}

/// Меняет только метаданные, не трогая время изменения: для служебных правок
/// вроде переименования тега.
pub(crate) fn update_meta(conn: &Connection, id: ItemId, meta: &[u8]) -> Result<()> {
    let changed = conn.execute(
        "UPDATE items SET meta = ?2, revision = revision + 1 WHERE id = ?1",
        params![id.as_bytes(), meta],
    )?;
    expect_one(changed)
}

pub(crate) fn set_deleted(
    conn: &Connection,
    id: ItemId,
    deleted_at: Option<i64>,
    now: i64,
) -> Result<()> {
    let changed = conn.execute(
        "UPDATE items SET deleted_at = ?2, updated_at = ?3, revision = revision + 1 WHERE id = ?1",
        params![id.as_bytes(), deleted_at, now],
    )?;
    expect_one(changed)
}

pub(crate) fn delete_item(conn: &Connection, id: ItemId) -> Result<()> {
    let changed = conn.execute("DELETE FROM items WHERE id = ?1", [id.as_bytes()])?;
    expect_one(changed)
}

/// Окончательно удаляет все записи из корзины. Возвращает их число.
pub(crate) fn delete_trashed(conn: &Connection) -> Result<usize> {
    Ok(conn.execute("DELETE FROM items WHERE deleted_at IS NOT NULL", [])?)
}

/// Строка таблицы `item_history`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HistoryRow {
    pub id: i64,
    pub data: Vec<u8>,
    pub created_at: i64,
}

pub(crate) fn insert_history(conn: &Connection, item: ItemId, data: &[u8], now: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO item_history (item_id, data, created_at) VALUES (?1, ?2, ?3)",
        params![item.as_bytes(), data, now],
    )?;
    Ok(())
}

/// Оставляет в истории записи только `keep` последних версий.
pub(crate) fn trim_history(conn: &Connection, item: ItemId, keep: usize) -> Result<()> {
    let keep = i64::try_from(keep).unwrap_or(i64::MAX);
    conn.execute(
        "DELETE FROM item_history WHERE item_id = ?1 AND id NOT IN (
           SELECT id FROM item_history WHERE item_id = ?1 ORDER BY id DESC LIMIT ?2)",
        params![item.as_bytes(), keep],
    )?;
    Ok(())
}

/// История записи, новые версии первыми.
pub(crate) fn list_history(conn: &Connection, item: ItemId) -> Result<Vec<HistoryRow>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, data, created_at FROM item_history WHERE item_id = ?1 ORDER BY id DESC",
    )?;
    let rows = stmt
        .query_map([item.as_bytes()], |r| {
            Ok(HistoryRow {
                id: r.get(0)?,
                data: r.get(1)?,
                created_at: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub(crate) fn record_use(conn: &Connection, item: ItemId, now: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO item_usage (item_id, use_count, last_used_at) VALUES (?1, 1, ?2)
         ON CONFLICT (item_id) DO UPDATE SET use_count = use_count + 1, last_used_at = ?2",
        params![item.as_bytes(), now],
    )?;
    Ok(())
}

fn expect_one(changed: usize) -> Result<()> {
    if changed == 1 {
        Ok(())
    } else {
        Err(Error::ItemNotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp() -> (TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.xpv");
        (dir, path)
    }

    fn pragma_i64(conn: &Connection, name: &str) -> i64 {
        conn.pragma_query_value(None, name, |r| r.get(0)).unwrap()
    }

    #[test]
    fn create_sets_up_schema_and_pragmas() {
        let (_dir, path) = temp();
        let conn = create(&path).unwrap();
        assert_eq!(pragma_i64(&conn, "user_version"), SCHEMA_VERSION);
        assert_eq!(
            pragma_i64(&conn, "application_id"),
            i64::from(APPLICATION_ID)
        );
        assert_eq!(pragma_i64(&conn, "foreign_keys"), 1);
        assert_eq!(pragma_i64(&conn, "secure_delete"), 1);
        assert_eq!(pragma_i64(&conn, "synchronous"), 2);
        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            tables,
            ["item_history", "item_usage", "items", "key_slots", "vault"]
        );
    }

    #[test]
    fn create_refuses_existing_file() {
        let (_dir, path) = temp();
        std::fs::write(&path, b"precious").unwrap();
        assert!(matches!(create(&path), Err(Error::AlreadyExists)));
        assert_eq!(std::fs::read(&path).unwrap(), b"precious");
    }

    #[test]
    fn create_makes_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a").join("b").join("vault.xpv");
        create(&path).unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn reopen_is_idempotent() {
        let (_dir, path) = temp();
        drop(create(&path).unwrap());
        drop(open(&path).unwrap());
        let conn = open(&path).unwrap();
        assert_eq!(pragma_i64(&conn, "user_version"), SCHEMA_VERSION);
    }

    #[test]
    fn open_missing() {
        let (_dir, path) = temp();
        assert!(matches!(open(&path), Err(Error::VaultNotFound)));
        assert!(!path.exists(), "open не должен создавать файл");
    }

    #[test]
    fn open_rejects_foreign_files() {
        let (dir, path) = temp();

        std::fs::write(&path, b"").unwrap();
        assert!(matches!(open(&path), Err(Error::NotAVault)));

        std::fs::write(&path, vec![0x5a; 8192]).unwrap();
        assert!(matches!(open(&path), Err(Error::NotAVault)));

        // Обычная база SQLite другого приложения.
        let other = dir.path().join("other.db");
        Connection::open(&other)
            .unwrap()
            .execute_batch("CREATE TABLE t (x); INSERT INTO t VALUES (1);")
            .unwrap();
        assert!(matches!(open(&other), Err(Error::NotAVault)));
    }

    #[test]
    fn open_rejects_newer_schema() {
        let (_dir, path) = temp();
        let conn = create(&path).unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        drop(conn);
        assert!(matches!(open(&path), Err(Error::UnsupportedVersion)));
    }

    #[test]
    fn open_migrates_old_schema() {
        // Файл XonkyPass до первой миграции: заголовок SQLite и application_id есть,
        // таблиц ещё нет.
        let (_dir, path) = temp();
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "application_id", APPLICATION_ID)
            .unwrap();
        drop(conn);

        let conn = open(&path).unwrap();
        assert_eq!(pragma_i64(&conn, "user_version"), SCHEMA_VERSION);
        let count: i64 = conn
            .query_row("SELECT count(*) FROM items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn schema_enforces_types() {
        let (_dir, path) = temp();
        let conn = create(&path).unwrap();
        // STRICT-таблица не примет текст вместо даты, CHECK — короткий id.
        assert!(
            conn.execute(
                "INSERT INTO items (id, meta, secret, created_at, updated_at)
                 VALUES (zeroblob(16), x'00', x'00', 'вчера', 0)",
                [],
            )
            .is_err()
        );
        assert!(
            conn.execute(
                "INSERT INTO items (id, meta, secret, created_at, updated_at)
                 VALUES (zeroblob(4), x'00', x'00', 0, 0)",
                [],
            )
            .is_err()
        );
    }

    #[test]
    fn history_is_trimmed_and_cascades() {
        let (_dir, path) = temp();
        let conn = create(&path).unwrap();
        let id = ItemId::generate();
        insert_item(&conn, id, b"m", b"s", 1).unwrap();
        for i in 0..10 {
            insert_history(&conn, id, &[i], i64::from(i)).unwrap();
        }
        trim_history(&conn, id, 3).unwrap();
        let history = list_history(&conn, id).unwrap();
        let data: Vec<u8> = history.iter().map(|h| h.data[0]).collect();
        assert_eq!(data, [9, 8, 7]);

        record_use(&conn, id, 5).unwrap();
        record_use(&conn, id, 6).unwrap();
        let row = get_item(&conn, id).unwrap().unwrap();
        assert_eq!((row.use_count, row.last_used_at), (2, Some(6)));

        delete_item(&conn, id).unwrap();
        assert!(list_history(&conn, id).unwrap().is_empty());
        let usage: i64 = conn
            .query_row("SELECT count(*) FROM item_usage", [], |r| r.get(0))
            .unwrap();
        assert_eq!(usage, 0);
    }

    #[test]
    fn missing_rows_are_reported() {
        let (_dir, path) = temp();
        let conn = create(&path).unwrap();
        let id = ItemId::generate();
        assert!(get_item(&conn, id).unwrap().is_none());
        assert!(matches!(
            update_content(&conn, id, b"m", b"s", 1),
            Err(Error::ItemNotFound)
        ));
        assert!(matches!(delete_item(&conn, id), Err(Error::ItemNotFound)));
        assert!(matches!(read_header(&conn), Err(Error::Corrupted(_))));
    }

    #[test]
    fn remove_files_cleans_sidecars() {
        let (_dir, path) = temp();
        drop(create(&path).unwrap());
        let mut wal = path.clone().into_os_string();
        wal.push("-wal");
        std::fs::write(&wal, b"x").unwrap();
        remove_files(&path);
        assert!(!path.exists());
        assert!(!PathBuf::from(wal).exists());
    }
}
