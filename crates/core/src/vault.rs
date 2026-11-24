//! Хранилище: создание, разблокировка, записи, корзина, история.
//!
//! [`Vault`] — это разблокированное хранилище. Заблокировать его значит удалить
//! значение: ключ при этом затирается, файл закрывается.

use std::fmt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::FORMAT_VERSION;
use crate::crypto::{self, Aad, Key};
use crate::error::{Error, Invalid, Result};
use crate::kdf::KdfParams;
use crate::keyslot::{self, Slot, SlotKind};
use crate::model::{self, CONTENT_VERSION, ItemId, ItemMeta, ItemSecret};
use crate::recovery::RecoveryKey;
use crate::search::{self, SearchIndex, SearchOptions, SearchResult, TagCount};
use crate::store::{self, Header, HistoryRow, ItemRow};

/// Сколько прошлых версий одной записи хранится в истории.
pub const HISTORY_LIMIT: usize = 50;

/// Чем было разблокировано хранилище.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnlockMethod {
    Password,
    RecoveryKey,
}

/// Какие записи показывать.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Обычные записи.
    Active,
    /// Записи в корзине.
    Trash,
}

/// Запись без секретов: для списка, поиска и карточки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemSummary {
    pub id: ItemId,
    pub meta: ItemMeta,
    /// Unix-время в миллисекундах.
    pub created_at: i64,
    pub updated_at: i64,
    /// Когда запись перенесена в корзину.
    pub deleted_at: Option<i64>,
    pub revision: i64,
    pub use_count: i64,
    pub last_used_at: Option<i64>,
}

/// Список записей. Записи, которые не удалось расшифровать или прочитать,
/// не прячутся молча, а перечисляются в `damaged`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ItemList {
    pub items: Vec<ItemSummary>,
    pub damaged: Vec<ItemId>,
}

/// Прошлая версия записи.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryEntry {
    pub id: i64,
    /// Когда эта версия была сохранена.
    pub updated_at: i64,
    /// Когда её заменила более новая.
    pub replaced_at: i64,
    pub meta: ItemMeta,
    pub secret: ItemSecret,
}

/// Снимок версии записи для истории.
#[derive(Serialize)]
struct SnapshotRef<'a> {
    v: u32,
    updated_at: i64,
    meta: &'a ItemMeta,
    secret: &'a ItemSecret,
}

#[derive(Deserialize)]
struct Snapshot {
    v: u32,
    updated_at: i64,
    meta: ItemMeta,
    secret: ItemSecret,
}

#[derive(Debug, Clone, Copy)]
enum Field {
    Meta,
    Secret,
}

impl Field {
    fn as_str(self) -> &'static str {
        match self {
            Field::Meta => "meta",
            Field::Secret => "secret",
        }
    }
}

/// Разблокированное хранилище.
pub struct Vault {
    conn: Connection,
    key: Key,
    vault_id: [u8; 16],
    unlocked_with: UnlockMethod,
    /// Метаданные записей вне корзины для поиска; строится при разблокировке.
    index: SearchIndex,
}

impl fmt::Debug for Vault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vault")
            .field("unlocked_with", &self.unlocked_with)
            .finish_non_exhaustive()
    }
}

impl Vault {
    /// Есть ли файл хранилища по этому пути.
    pub fn exists(path: &Path) -> bool {
        path.is_file()
    }

    /// Создаёт хранилище и возвращает его вместе с ключом восстановления.
    ///
    /// Ключ восстановления нужно показать пользователю один раз: он нигде не хранится.
    /// Параметры KDF обычно берутся из [`KdfParams::calibrate`].
    pub fn create(path: &Path, password: &str, kdf: KdfParams) -> Result<(Self, RecoveryKey)> {
        check_password(password)?;
        if path.exists() {
            return Err(Error::AlreadyExists);
        }

        let mut vault_id = [0u8; 16];
        crypto::fill_random(&mut vault_id);
        let key = Key::random();
        let recovery = RecoveryKey::generate();

        // Медленный вывод ключей — до создания файла, чтобы не оставлять его полупустым.
        let password_slot = keyslot::seal(
            SlotKind::Password,
            password.as_bytes(),
            kdf,
            &key,
            &vault_id,
        )?;
        let recovery_slot = keyslot::seal(
            SlotKind::Recovery,
            recovery.as_bytes(),
            KdfParams::RECOVERY,
            &key,
            &vault_id,
        )?;
        let header = Header {
            vault_id,
            format_ver: FORMAT_VERSION,
            created_at: now_ms(),
        };

        let created = (|| -> Result<Connection> {
            let mut conn = store::create(path)?;
            let tx = conn.transaction()?;
            store::insert_header(&tx, &header)?;
            store::put_slot(&tx, &password_slot)?;
            store::put_slot(&tx, &recovery_slot)?;
            tx.commit()?;
            Ok(conn)
        })();
        let conn = match created {
            Ok(conn) => conn,
            // Чужой файл, появившийся за это время, не трогаем.
            Err(Error::AlreadyExists) => return Err(Error::AlreadyExists),
            Err(err) => {
                store::remove_files(path);
                return Err(err);
            }
        };

        let vault = Self {
            conn,
            key,
            vault_id,
            unlocked_with: UnlockMethod::Password,
            index: SearchIndex::default(),
        };
        Ok((vault, recovery))
    }

    /// Разблокирует хранилище мастер-паролем.
    pub fn unlock(path: &Path, password: &str) -> Result<Self> {
        Self::unlock_slot(
            path,
            SlotKind::Password,
            password.as_bytes(),
            UnlockMethod::Password,
        )
    }

    /// Разблокирует хранилище ключом восстановления. После этого можно задать
    /// новый мастер-пароль через [`Vault::reset_password`].
    pub fn unlock_with_recovery_key(path: &Path, key: &RecoveryKey) -> Result<Self> {
        Self::unlock_slot(
            path,
            SlotKind::Recovery,
            key.as_bytes(),
            UnlockMethod::RecoveryKey,
        )
    }

    fn unlock_slot(
        path: &Path,
        kind: SlotKind,
        secret: &[u8],
        method: UnlockMethod,
    ) -> Result<Self> {
        let conn = store::open(path)?;
        let header = store::read_header(&conn)?;
        if header.format_ver > FORMAT_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        if header.format_ver != FORMAT_VERSION {
            return Err(Error::Corrupted(format!(
                "версия формата {}",
                header.format_ver
            )));
        }
        let slot = read_slot(&conn, kind)?;
        let key = keyslot::open(&slot, secret, &header.vault_id)?;
        let mut vault = Self {
            conn,
            key,
            vault_id: header.vault_id,
            unlocked_with: method,
            index: SearchIndex::default(),
        };
        vault.index = SearchIndex::new(vault.items(Scope::Active)?.items);
        Ok(vault)
    }

    /// Блокирует хранилище: затирает ключ и закрывает файл.
    pub fn lock(self) {}

    pub fn unlocked_with(&self) -> UnlockMethod {
        self.unlocked_with
    }

    /// Параметры Argon2 слота мастер-пароля.
    pub fn kdf_params(&self) -> Result<KdfParams> {
        keyslot::kdf_params(&read_slot(&self.conn, SlotKind::Password)?)
    }

    /// Проверяет мастер-пароль, не меняя состояние (например, перед показом
    /// нового ключа восстановления).
    pub fn verify_password(&self, password: &str) -> Result<()> {
        let slot = read_slot(&self.conn, SlotKind::Password)?;
        keyslot::open(&slot, password.as_bytes(), &self.vault_id).map(drop)
    }

    /// Меняет мастер-пароль. Перешифровывается только слот пароля.
    pub fn change_password(&mut self, current: &str, new: &str, kdf: KdfParams) -> Result<()> {
        self.verify_password(current)?;
        self.set_password(new, kdf)
    }

    /// Задаёт новый мастер-пароль без старого. Доступно, только если хранилище
    /// разблокировано ключом восстановления.
    pub fn reset_password(&mut self, new: &str, kdf: KdfParams) -> Result<()> {
        if self.unlocked_with != UnlockMethod::RecoveryKey {
            return Err(Error::NotAllowed(
                "сброс пароля доступен только после входа по ключу восстановления",
            ));
        }
        self.set_password(new, kdf)
    }

    fn set_password(&mut self, new: &str, kdf: KdfParams) -> Result<()> {
        check_password(new)?;
        let slot = keyslot::seal(
            SlotKind::Password,
            new.as_bytes(),
            kdf,
            &self.key,
            &self.vault_id,
        )?;
        store::put_slot(&self.conn, &slot)
    }

    /// Заменяет ключ восстановления новым; старый перестаёт работать.
    pub fn regenerate_recovery_key(&mut self) -> Result<RecoveryKey> {
        let recovery = RecoveryKey::generate();
        let slot = keyslot::seal(
            SlotKind::Recovery,
            recovery.as_bytes(),
            KdfParams::RECOVERY,
            &self.key,
            &self.vault_id,
        )?;
        store::put_slot(&self.conn, &slot)?;
        Ok(recovery)
    }

    /// Создаёт запись.
    pub fn create_item(&mut self, mut meta: ItemMeta, mut secret: ItemSecret) -> Result<ItemId> {
        meta.normalize()?;
        secret.normalize()?;
        sync_has_totp(&mut meta, &secret);

        let id = ItemId::generate();
        let meta_ct = self.seal_field(id, Field::Meta, &model::encode_meta(&meta));
        let secret_ct = self.seal_field(id, Field::Secret, &model::encode_secret(&secret));
        store::insert_item(&self.conn, id, &meta_ct, &secret_ct, now_ms())?;
        self.reindex(id)?;
        Ok(id)
    }

    /// Изменяет запись. Если `secret` — `None`, секретная часть остаётся прежней:
    /// так интерфейсу не нужно расшифровывать пароль, чтобы, например, отметить
    /// запись избранной.
    ///
    /// Прошлая версия попадает в историю, если изменилось содержимое; смена только
    /// тегов или отметки «избранное» историю не засоряет.
    pub fn update_item(
        &mut self,
        id: ItemId,
        mut meta: ItemMeta,
        secret: Option<ItemSecret>,
    ) -> Result<()> {
        meta.normalize()?;
        let new_secret = match secret {
            Some(mut secret) => {
                secret.normalize()?;
                Some(secret)
            }
            None => None,
        };

        let row = self.row(id)?;
        let old_meta = self.decrypt_meta(&row)?;
        let old_secret = self.decrypt_secret(&row)?;
        if meta.kind() != old_meta.kind() {
            return Err(Invalid::KindChanged.into());
        }
        sync_has_totp(&mut meta, new_secret.as_ref().unwrap_or(&old_secret));

        let secret_changed = new_secret.as_ref().is_some_and(|s| *s != old_secret);
        if meta == old_meta && !secret_changed {
            return Ok(());
        }

        let now = now_ms();
        let meta_ct = self.seal_field(id, Field::Meta, &model::encode_meta(&meta));
        let secret_ct = match &new_secret {
            Some(secret) if secret_changed => {
                self.seal_field(id, Field::Secret, &model::encode_secret(secret))
            }
            _ => row.secret.clone(),
        };

        let tx = self.conn.transaction()?;
        if secret_changed || content_changed(&old_meta, &meta) {
            let snapshot = seal_snapshot(
                &self.key,
                &self.vault_id,
                id,
                now,
                &old_meta,
                &old_secret,
                row.updated_at,
            );
            store::insert_history(&tx, id, &snapshot, now)?;
            store::trim_history(&tx, id, HISTORY_LIMIT)?;
        }
        store::update_content(&tx, id, &meta_ct, &secret_ct, now)?;
        tx.commit()?;
        self.reindex(id)
    }

    /// Запись без секретов.
    pub fn item(&self, id: ItemId) -> Result<ItemSummary> {
        let row = self.row(id)?;
        self.summary(row)
    }

    /// Все записи вне корзины или в корзине, в порядке создания.
    pub fn items(&self, scope: Scope) -> Result<ItemList> {
        let rows = store::list_items(&self.conn, scope == Scope::Trash)?;
        let mut list = ItemList::default();
        for row in rows {
            let id = row.id;
            match self.summary(row) {
                Ok(summary) => list.items.push(summary),
                Err(Error::Damaged(_) | Error::Corrupted(_) | Error::UnsupportedVersion) => {
                    list.damaged.push(id);
                }
                Err(err) => return Err(err),
            }
        }
        Ok(list)
    }

    /// Секретная часть записи. Вызывается только по явному действию пользователя.
    pub fn reveal(&self, id: ItemId) -> Result<ItemSecret> {
        let row = self.row(id)?;
        self.decrypt_secret(&row)
    }

    /// Переносит запись в корзину. Повторный вызов ничего не делает.
    pub fn trash(&mut self, id: ItemId) -> Result<()> {
        let row = self.row(id)?;
        if row.deleted_at.is_some() {
            return Ok(());
        }
        let now = now_ms();
        store::set_deleted(&self.conn, id, Some(now), now)?;
        self.index.remove(id);
        Ok(())
    }

    /// Возвращает запись из корзины. Для записи не в корзине ничего не делает.
    pub fn restore(&mut self, id: ItemId) -> Result<()> {
        let row = self.row(id)?;
        if row.deleted_at.is_none() {
            return Ok(());
        }
        store::set_deleted(&self.conn, id, None, now_ms())?;
        self.reindex(id)
    }

    /// Удаляет запись из корзины навсегда, вместе с историей.
    pub fn purge(&mut self, id: ItemId) -> Result<()> {
        let row = self.row(id)?;
        if row.deleted_at.is_none() {
            return Err(Error::NotInTrash);
        }
        store::delete_item(&self.conn, id)
    }

    /// Очищает корзину. Возвращает число удалённых записей.
    pub fn empty_trash(&mut self) -> Result<usize> {
        store::delete_trashed(&self.conn)
    }

    /// Прошлые версии записи, новые первыми. Содержат секреты: вызывается только
    /// по явному действию пользователя.
    pub fn history(&self, id: ItemId) -> Result<Vec<HistoryEntry>> {
        self.row(id)?;
        store::list_history(&self.conn, id)?
            .iter()
            .map(|row| self.open_snapshot(id, row))
            .collect()
    }

    /// Отмечает, что запись использовали (скопировали пароль и т. п.).
    pub fn record_use(&mut self, id: ItemId) -> Result<()> {
        self.row(id)?;
        let now = now_ms();
        store::record_use(&self.conn, id, now)?;
        self.index.record_use(id, now);
        Ok(())
    }

    /// Ищет записи вне корзины. Синтаксис запроса описан в модуле [`search`].
    pub fn search(&self, query: &str, options: &SearchOptions) -> SearchResult {
        self.index.search(query, options)
    }

    /// Теги записей вне корзины с числом записей, самые частые первыми.
    pub fn tags(&self) -> Vec<TagCount> {
        self.index.tags().to_vec()
    }

    /// Переименовывает тег во всех записях, в том числе в корзине. Если тег с новым
    /// именем уже есть, теги сливаются. В заметках меняется и `#тег` в тексте.
    /// Время изменения записей не меняется и в историю они не попадают.
    /// Возвращает число изменённых записей.
    pub fn rename_tag(&mut self, from: &str, to: &str) -> Result<usize> {
        let to = model::parse_tag(to)?;
        self.retag(from, Some(&to))
    }

    /// Убирает тег из всех записей, в том числе из корзины. В заметках `#тег`
    /// превращается в обычное слово, чтобы тег не вернулся при следующем сохранении.
    /// Возвращает число изменённых записей.
    pub fn delete_tag(&mut self, tag: &str) -> Result<usize> {
        self.retag(tag, None)
    }

    fn retag(&mut self, from: &str, to: Option<&str>) -> Result<usize> {
        let from = model::parse_tag(from)?.to_lowercase();
        let mut rows = store::list_items(&self.conn, false)?;
        rows.extend(store::list_items(&self.conn, true)?);

        let mut changed = Vec::new();
        for row in &rows {
            let old = match self.decrypt_meta(row) {
                Ok(meta) => meta,
                // Повреждённые записи не трогаем: их и так нельзя открыть.
                Err(Error::Damaged(_) | Error::Corrupted(_) | Error::UnsupportedVersion) => {
                    continue;
                }
                Err(err) => return Err(err),
            };
            let mut meta = old.clone();
            retag_meta(&mut meta, &from, to);
            meta.normalize()?;
            if meta != old {
                let meta_ct = self.seal_field(row.id, Field::Meta, &model::encode_meta(&meta));
                changed.push((row.id, meta_ct));
            }
        }

        let tx = self.conn.transaction()?;
        for (id, meta_ct) in &changed {
            store::update_meta(&tx, *id, meta_ct)?;
        }
        tx.commit()?;
        for (id, _) in &changed {
            self.reindex(*id)?;
        }
        Ok(changed.len())
    }

    /// Обновляет запись в поисковом индексе после изменения в файле.
    fn reindex(&mut self, id: ItemId) -> Result<()> {
        let item = self.item(id)?;
        self.index.upsert(item);
        Ok(())
    }

    fn row(&self, id: ItemId) -> Result<ItemRow> {
        store::get_item(&self.conn, id)?.ok_or(Error::ItemNotFound)
    }

    fn summary(&self, row: ItemRow) -> Result<ItemSummary> {
        let meta = self.decrypt_meta(&row)?;
        Ok(ItemSummary {
            id: row.id,
            meta,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
            revision: row.revision,
            use_count: row.use_count,
            last_used_at: row.last_used_at,
        })
    }

    fn item_aad(&self, id: ItemId, field: Field) -> Aad {
        Aad::new("item")
            .part(&self.vault_id)
            .part(id.as_bytes())
            .part(field.as_str().as_bytes())
            .part(&FORMAT_VERSION.to_le_bytes())
    }

    fn seal_field(&self, id: ItemId, field: Field, plaintext: &[u8]) -> Vec<u8> {
        crypto::seal(&self.key, &self.item_aad(id, field), plaintext)
    }

    fn open_field(&self, id: ItemId, field: Field, data: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        crypto::open(&self.key, &self.item_aad(id, field), data).map_err(|_| Error::Damaged(id))
    }

    fn decrypt_meta(&self, row: &ItemRow) -> Result<ItemMeta> {
        model::decode_meta(&self.open_field(row.id, Field::Meta, &row.meta)?)
    }

    fn decrypt_secret(&self, row: &ItemRow) -> Result<ItemSecret> {
        model::decode_secret(&self.open_field(row.id, Field::Secret, &row.secret)?)
    }

    fn open_snapshot(&self, id: ItemId, row: &HistoryRow) -> Result<HistoryEntry> {
        let aad = history_aad(&self.vault_id, id, row.created_at);
        let plain = crypto::open(&self.key, &aad, &row.data).map_err(|_| Error::Damaged(id))?;
        let snapshot: Snapshot = serde_json::from_slice(&plain).map_err(|_| Error::Damaged(id))?;
        if snapshot.v != CONTENT_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        Ok(HistoryEntry {
            id: row.id,
            updated_at: snapshot.updated_at,
            replaced_at: row.created_at,
            meta: snapshot.meta,
            secret: snapshot.secret,
        })
    }
}

fn read_slot(conn: &Connection, kind: SlotKind) -> Result<Slot> {
    store::get_slot(conn, kind)?
        .ok_or_else(|| Error::Corrupted(format!("нет слота ключа «{}»", kind.as_str())))
}

fn check_password(password: &str) -> Result<()> {
    if password.is_empty() {
        return Err(Invalid::EmptyPassword.into());
    }
    Ok(())
}

/// `has_totp` в метаданных всегда отражает секретную часть.
fn sync_has_totp(meta: &mut ItemMeta, secret: &ItemSecret) {
    if let ItemMeta::Login(login) = meta {
        login.has_totp = !secret.totp.is_empty();
    }
}

/// Меняет тег `from` (в нижнем регистре) на `to` или убирает его.
fn retag_meta(meta: &mut ItemMeta, from: &str, to: Option<&str>) {
    let (tags, body) = match meta {
        ItemMeta::Login(m) => (&mut m.tags, None),
        ItemMeta::Note(m) => (&mut m.tags, Some(&mut m.body)),
    };
    if let Some(i) = tags.iter().position(|t| t.to_lowercase() == from) {
        match to {
            Some(to) => to.clone_into(&mut tags[i]),
            None => {
                tags.remove(i);
            }
        }
    }
    if let Some(body) = body
        && let Some(new) = search::rewrite_hashtags(body, from, to)
    {
        *body = new;
    }
}

/// Изменилось ли что-то, кроме тегов и отметки «избранное».
fn content_changed(old: &ItemMeta, new: &ItemMeta) -> bool {
    fn strip(meta: &ItemMeta) -> ItemMeta {
        let mut meta = meta.clone();
        match &mut meta {
            ItemMeta::Login(m) => {
                m.tags.clear();
                m.favorite = false;
            }
            ItemMeta::Note(m) => {
                m.tags.clear();
                m.favorite = false;
            }
        }
        meta
    }
    strip(old) != strip(new)
}

fn history_aad(vault_id: &[u8], id: ItemId, created_at: i64) -> Aad {
    Aad::new("history")
        .part(vault_id)
        .part(id.as_bytes())
        .part(&created_at.to_le_bytes())
        .part(&FORMAT_VERSION.to_le_bytes())
}

fn seal_snapshot(
    key: &Key,
    vault_id: &[u8],
    id: ItemId,
    now: i64,
    meta: &ItemMeta,
    secret: &ItemSecret,
    updated_at: i64,
) -> Vec<u8> {
    let snapshot = SnapshotRef {
        v: CONTENT_VERSION,
        updated_at,
        meta,
        secret,
    };
    let plain = Zeroizing::new(serde_json::to_vec(&snapshot).expect("снимок записи сериализуется"));
    crypto::seal(key, &history_aad(vault_id, id, now), &plain)
}

/// Текущее время, unix, мс.
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
