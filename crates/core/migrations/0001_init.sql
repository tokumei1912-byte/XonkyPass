-- Схема хранилища XonkyPass, версия 1. Описание — docs/FORMAT.md.
-- Открыто лежат только идентификаторы, даты и счётчики; всё остальное зашифровано.

CREATE TABLE vault (
  id           INTEGER PRIMARY KEY CHECK (id = 1),
  vault_id     BLOB    NOT NULL CHECK (length(vault_id) = 16),
  format_ver   INTEGER NOT NULL,
  created_at   INTEGER NOT NULL
) STRICT;

CREATE TABLE key_slots (
  kind         TEXT PRIMARY KEY,           -- 'password' | 'recovery' | позже 'hello'
  params       TEXT NOT NULL,              -- JSON: алгоритм, параметры KDF, соль
  wrapped_key  BLOB NOT NULL               -- nonce ‖ шифр(ключ хранилища) ‖ тег
) STRICT;

CREATE TABLE items (
  id           BLOB PRIMARY KEY CHECK (length(id) = 16),  -- UUIDv7
  meta         BLOB NOT NULL,              -- nonce ‖ шифр(JSON метаданных) ‖ тег
  secret       BLOB NOT NULL,              -- nonce ‖ шифр(JSON секретов) ‖ тег
  created_at   INTEGER NOT NULL,           -- unix, мс
  updated_at   INTEGER NOT NULL,
  deleted_at   INTEGER,                    -- не NULL = в корзине
  revision     INTEGER NOT NULL DEFAULT 1  -- задел под синхронизацию
) STRICT, WITHOUT ROWID;

CREATE TABLE item_history (
  id           INTEGER PRIMARY KEY,
  item_id      BLOB NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  data         BLOB NOT NULL,              -- nonce ‖ шифр(JSON снимка) ‖ тег
  created_at   INTEGER NOT NULL
) STRICT;

CREATE INDEX item_history_item ON item_history(item_id, id);

CREATE TABLE item_usage (                  -- для сортировки «часто / недавно»
  item_id      BLOB PRIMARY KEY REFERENCES items(id) ON DELETE CASCADE,
  use_count    INTEGER NOT NULL DEFAULT 0,
  last_used_at INTEGER
) STRICT, WITHOUT ROWID;
