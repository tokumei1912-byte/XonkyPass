//! Генератор паролей и оценка надёжности.

use xonkypass_core::generator::{self, PassphraseOptions, PasswordOptions, Strength};

use crate::error::CommandError;

#[tauri::command]
pub fn generate_password(options: PasswordOptions) -> Result<String, CommandError> {
    let password = generator::generate_password(&options)?;
    Ok(String::clone(&password))
}

#[tauri::command]
pub fn generate_passphrase(options: PassphraseOptions) -> Result<String, CommandError> {
    let phrase = generator::generate_passphrase(&options)?;
    Ok(String::clone(&phrase))
}

/// Оценка надёжности. `inputs` — название, логин, сайт: пароль из них считается слабым.
/// Асинхронная, чтобы оценка на каждое нажатие клавиши не занимала поток интерфейса.
#[tauri::command]
pub async fn password_strength(password: String, inputs: Vec<String>) -> Strength {
    let password = zeroize::Zeroizing::new(password);
    let inputs: Vec<&str> = inputs.iter().map(String::as_str).collect();
    generator::password_strength(&password, &inputs)
}
