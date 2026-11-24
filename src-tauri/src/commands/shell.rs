//! Ссылки, Проводник и сохранение ключа восстановления в файл.

use tauri::{AppHandle, Url, WebviewWindow};
use xonkypass_core::RecoveryKey;
use zeroize::Zeroizing;

use crate::error::CommandError;
use crate::platform;
use crate::platform::dialog::{self, SaveDialog};
use crate::state::blocking;

/// Открывает сайт из записи в браузере по умолчанию.
///
/// Синхронная команда: выполняется в главном потоке, где для оболочки уже есть COM.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), CommandError> {
    let url = web_url(&url).ok_or_else(|| CommandError::new("invalid", "это не адрес сайта"))?;
    platform::shell::open_url(url.as_str()).map_err(CommandError::internal)
}

/// Показывает файл хранилища в Проводнике.
#[tauri::command]
pub async fn reveal_vault(app: AppHandle) -> Result<(), CommandError> {
    blocking(&app, |_, state| {
        platform::shell::reveal(&state.vault_path).map_err(CommandError::internal)
    })
    .await
}

/// Сохраняет ключ восстановления в текстовый файл. Место выбирает пользователь
/// в системном окне; `false` — окно закрыли без сохранения.
#[tauri::command]
pub async fn recovery_key_save(
    app: AppHandle,
    window: WebviewWindow,
    key: String,
) -> Result<bool, CommandError> {
    let key = Zeroizing::new(key);
    // Команда пишет только ключ восстановления, а не любой текст из интерфейса.
    RecoveryKey::parse(&key)?;
    let owner = platform::window_handle(&window);
    blocking(&app, move |_, _| {
        let dialog = SaveDialog {
            title: "Сохранить ключ восстановления",
            file_name: "Ключ восстановления XonkyPass.txt",
            filter_name: "Текстовый файл",
            extension: "txt",
        };
        let Some(path) = dialog::save_file(owner, &dialog).map_err(CommandError::internal)? else {
            return Ok(false);
        };
        let text = Zeroizing::new(format!(
            "XonkyPass — ключ восстановления\r\n\r\n{}\r\n\r\n\
             Ключ открывает хранилище, если вы забудете мастер-пароль.\r\n\
             Храните его отдельно от компьютера: на бумаге или на флешке в надёжном месте.\r\n",
            key.trim()
        ));
        std::fs::write(&path, text.as_bytes()).map_err(|err| {
            tracing::warn!(%err, "не удалось сохранить ключ восстановления");
            CommandError::new("io", err.to_string())
        })?;
        tracing::info!("ключ восстановления сохранён в файл");
        Ok(true)
    })
    .await
}

/// Адрес сайта из записи: `github.com` → `https://github.com/`. Открываются только
/// http и https без логина и пароля в адресе: `file:`, `javascript:` и прочее из
/// записи в браузер не попадёт.
pub fn web_url(input: &str) -> Option<Url> {
    let input = input.trim();
    if input.is_empty() || input.chars().any(char::is_whitespace) {
        return None;
    }
    let explicit = Url::parse(input).ok().filter(is_web);
    let url = match explicit {
        Some(url) => url,
        None if input.contains("://") => return None,
        None => Url::parse(&format!("https://{input}")).ok()?,
    };
    is_web(&url).then_some(url)
}

fn is_web(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some_and(|host| !host.is_empty())
        && url.username().is_empty()
        && url.password().is_none()
}

#[cfg(test)]
mod tests {
    use super::web_url;

    fn open(input: &str) -> Option<String> {
        web_url(input).map(String::from)
    }

    #[test]
    fn bare_domains_get_https() {
        assert_eq!(open("github.com").as_deref(), Some("https://github.com/"));
        assert_eq!(
            open(" gist.github.com/user ").as_deref(),
            Some("https://gist.github.com/user")
        );
        assert_eq!(
            open("localhost:8080").as_deref(),
            Some("https://localhost:8080/")
        );
    }

    #[test]
    fn web_schemes_are_kept() {
        assert_eq!(
            open("http://192.168.0.1").as_deref(),
            Some("http://192.168.0.1/")
        );
        assert_eq!(
            open("HTTPS://Example.com/a?b=1").as_deref(),
            Some("https://example.com/a?b=1")
        );
    }

    #[test]
    fn other_schemes_and_tricks_are_refused() {
        for input in [
            "",
            "   ",
            "javascript:alert(1)",
            "file:///C:/Windows/System32/calc.exe",
            "ftp://example.com",
            "mailto:me@example.com",
            "https://user:pass@example.com",
            "evil.com@bank.example",
            "github .com",
            "ms-settings:privacy",
        ] {
            assert_eq!(open(input), None, "{input}");
        }
    }
}
