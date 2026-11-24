//! Заглушки оформления и оболочки для сборки вне Windows (проверки в Linux).

pub mod appearance {
    use crate::platform::{Accent, PlatformError, PlatformResult};

    pub fn apply_mica(_hwnd: isize) -> PlatformResult<()> {
        Err(PlatformError::unsupported())
    }

    pub fn accent() -> PlatformResult<Accent> {
        Err(PlatformError::unsupported())
    }
}

pub mod dialog {
    use std::path::PathBuf;

    use crate::platform::{PlatformError, PlatformResult};

    // Поля читает только реализация для Windows.
    #[allow(dead_code)]
    pub struct SaveDialog<'a> {
        pub title: &'a str,
        pub file_name: &'a str,
        pub filter_name: &'a str,
        pub extension: &'a str,
    }

    pub fn save_file(_owner: isize, _options: &SaveDialog<'_>) -> PlatformResult<Option<PathBuf>> {
        Err(PlatformError::unsupported())
    }
}

pub mod dock {
    use crate::dock::geometry::Monitor;

    /// Панелью у края управляет только Windows-версия.
    pub fn monitors() -> Vec<Monitor> {
        Vec::new()
    }
}

pub mod shell {
    use std::path::Path;

    use crate::platform::{PlatformError, PlatformResult};

    pub fn open_url(_url: &str) -> PlatformResult<()> {
        Err(PlatformError::unsupported())
    }

    pub fn reveal(_path: &Path) -> PlatformResult<()> {
        Err(PlatformError::unsupported())
    }
}
