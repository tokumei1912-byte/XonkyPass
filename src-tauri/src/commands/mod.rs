//! Команды IPC, доступные интерфейсу через `invoke()`.
//!
//! Тонкий слой над `xonkypass_core`: проверка, что хранилище открыто, перенос тяжёлой
//! работы в фоновые потоки и перевод ошибок в `{ code, message }`. Пароли попадают
//! в интерфейс только по явному действию (`item_reveal`), копирование в буфер
//! делает сам Rust (`item_copy`).

pub mod app;
pub mod clipboard;
pub mod dock;
pub mod generator;
pub mod items;
pub mod search;
pub mod settings;
pub mod shell;
pub mod vault;
