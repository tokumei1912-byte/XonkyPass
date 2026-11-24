//! Вывод ключа из пароля: Argon2id.

use std::time::{Duration, Instant};

use argon2::{Algorithm, Argon2, Version};
use serde::Serialize;
use zeroize::Zeroizing;

use crate::crypto::{KEY_LEN, Key};
use crate::error::{Error, Result};

/// Сколько должна длиться разблокировка на этом компьютере (`docs/PLAN.md`: 0,5–1 с).
pub const UNLOCK_TARGET: Duration = Duration::from_millis(600);

/// Параметры Argon2id.
///
/// Снаружи их можно получить только как [`KdfParams::DEFAULT`] или через
/// [`KdfParams::calibrate`], поэтому слабые параметры для мастер-пароля задать нельзя.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct KdfParams {
    m_kib: u32,
    t: u32,
    p: u32,
}

impl KdfParams {
    /// Нижняя граница для мастер-пароля: 64 МиБ памяти, 3 прохода, 4 потока.
    pub const DEFAULT: Self = Self {
        m_kib: 64 * 1024,
        t: 3,
        p: 4,
    };

    /// Для ключа восстановления. В нём 128 случайных бит, перебирать его бессмысленно,
    /// поэтому Argon2id здесь лёгкий и нужен только для единообразия слотов.
    pub(crate) const RECOVERY: Self = Self {
        m_kib: 19 * 1024,
        t: 2,
        p: 1,
    };

    /// Пределы для параметров, прочитанных из файла: испорченный или подложенный
    /// файл не должен заставить приложение выделить гигабайты памяти или зависнуть.
    const MAX_M_KIB: u32 = 1024 * 1024;
    const MAX_T: u32 = 256;
    const MAX_P: u32 = 16;
    /// Предел работы (память × проходы): вчетверо больше, чем может выбрать калибровка.
    const MAX_WORK: u64 = 64 * 1024 * 4 * Self::MAX_CALIBRATED_T as u64;

    /// Наибольшее число проходов, которое выбирает калибровка. Многопоточный Argon2id
    /// на современном процессоре делает проход по 64 МиБ примерно за 10 мс, поэтому
    /// для бюджета 0,5–1 с нужно несколько десятков проходов.
    const MAX_CALIBRATED_T: u32 = 64;

    pub(crate) const fn new_unchecked(m_kib: u32, t: u32, p: u32) -> Self {
        Self { m_kib, t, p }
    }

    /// Память в КиБ.
    pub fn m_kib(&self) -> u32 {
        self.m_kib
    }

    /// Число проходов.
    pub fn t(&self) -> u32 {
        self.t
    }

    /// Число потоков (полос).
    pub fn p(&self) -> u32 {
        self.p
    }

    /// Подбирает число проходов так, чтобы вывод ключа на этом компьютере занимал
    /// не меньше `target`. Память и потоки берутся из [`KdfParams::DEFAULT`],
    /// ниже `DEFAULT` параметры не опускаются даже на медленном компьютере.
    pub fn calibrate(target: Duration) -> Self {
        let base = Self::DEFAULT;
        // Первый замер включает выделение памяти и запуск потоков, берём лучший из двух.
        let elapsed = (0..2)
            .map(|_| {
                let start = Instant::now();
                let _ = derive_key(b"calibration", &[0u8; 16], base);
                start.elapsed()
            })
            .min()
            .unwrap_or(Duration::ZERO);
        Self {
            t: passes_for(target, elapsed, base.t),
            ..base
        }
    }

    /// Проверка параметров, прочитанных из файла.
    pub(crate) fn validate(&self) -> Result<()> {
        let ok = (1..=Self::MAX_P).contains(&self.p)
            && (1..=Self::MAX_T).contains(&self.t)
            && self.m_kib <= Self::MAX_M_KIB
            && self.m_kib >= 8 * self.p
            && u64::from(self.m_kib) * u64::from(self.t) <= Self::MAX_WORK;
        if ok {
            Ok(())
        } else {
            Err(Error::Corrupted(format!(
                "недопустимые параметры Argon2id: m={} КиБ, t={}, p={}",
                self.m_kib, self.t, self.p
            )))
        }
    }
}

/// Время Argon2 растёт линейно с числом проходов: по замеру на `base` проходах
/// считаем, сколько их нужно для `target`.
fn passes_for(target: Duration, elapsed: Duration, base: u32) -> u32 {
    if elapsed.is_zero() || elapsed >= target {
        return base;
    }
    let ratio = target.as_secs_f64() / elapsed.as_secs_f64();
    let passes = (f64::from(base) * ratio).ceil();
    // `passes` конечно и положительно: elapsed > 0 и elapsed < target.
    let passes = passes.min(f64::from(KdfParams::MAX_CALIBRATED_T)) as u32;
    passes.max(base)
}

/// Выводит ключ из секрета (пароля или ключа восстановления).
pub(crate) fn derive_key(secret: &[u8], salt: &[u8], params: KdfParams) -> Result<Key> {
    params.validate()?;
    let argon_params =
        argon2::Params::new(params.m_kib, params.t, params.p, Some(KEY_LEN)).map_err(Error::Kdf)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    argon
        .hash_password_into(secret, salt, out.as_mut())
        .map_err(Error::Kdf)?;
    Ok(Key::from_bytes(out))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::crypto::{Aad, open, seal};

    /// Быстрые параметры для тестов хранилища.
    pub(crate) const FAST: KdfParams = KdfParams::new_unchecked(64, 1, 1);

    /// Ключи равны, если один открывает то, что зашифровано другим.
    fn same_key(a: &Key, b: &Key) -> bool {
        let aad = Aad::new("test");
        open(b, &aad, &seal(a, &aad, b"x")).is_ok()
    }

    #[test]
    fn deterministic() {
        let a = derive_key(b"password", &[1; 16], FAST).unwrap();
        let b = derive_key(b"password", &[1; 16], FAST).unwrap();
        assert!(same_key(&a, &b));
    }

    #[test]
    fn salt_and_password_matter() {
        let base = derive_key(b"password", &[1; 16], FAST).unwrap();
        let other_salt = derive_key(b"password", &[2; 16], FAST).unwrap();
        let other_password = derive_key(b"Password", &[1; 16], FAST).unwrap();
        assert!(!same_key(&base, &other_salt));
        assert!(!same_key(&base, &other_password));
    }

    #[test]
    fn params_matter() {
        let a = derive_key(b"password", &[1; 16], FAST).unwrap();
        let b = derive_key(b"password", &[1; 16], KdfParams::new_unchecked(64, 2, 1)).unwrap();
        assert!(!same_key(&a, &b));
    }

    #[test]
    fn rejects_dangerous_params() {
        for params in [
            KdfParams::new_unchecked(2 * 1024 * 1024, 3, 4),
            KdfParams::new_unchecked(65536, 0, 4),
            KdfParams::new_unchecked(65536, 1000, 4),
            KdfParams::new_unchecked(65536, 3, 0),
            KdfParams::new_unchecked(65536, 3, 64),
            KdfParams::new_unchecked(16, 3, 4),
            // Каждый параметр в пределах, но вместе — минуты работы.
            KdfParams::new_unchecked(1024 * 1024, 64, 4),
        ] {
            assert!(
                matches!(
                    derive_key(b"pw", &[0; 16], params),
                    Err(Error::Corrupted(_))
                ),
                "{params:?}"
            );
        }
    }

    #[test]
    fn passes_for_scales_linearly() {
        let ms = Duration::from_millis;
        assert_eq!(passes_for(ms(600), ms(200), 3), 9);
        assert_eq!(passes_for(ms(600), ms(250), 3), 8);
        // Медленный компьютер: не опускаемся ниже базы.
        assert_eq!(passes_for(ms(600), ms(2000), 3), 3);
        // Очень быстрый: не выше предела.
        assert_eq!(passes_for(ms(600), ms(1), 3), KdfParams::MAX_CALIBRATED_T);
        assert_eq!(passes_for(ms(600), Duration::ZERO, 3), 3);
    }

    #[test]
    fn calibrate_never_goes_below_default() {
        let params = KdfParams::calibrate(Duration::from_millis(50));
        let base = KdfParams::DEFAULT;
        assert_eq!((params.m_kib, params.p), (base.m_kib, base.p));
        assert!(params.t >= base.t);
        assert!(params.validate().is_ok());
    }

    #[test]
    fn default_and_recovery_params_are_valid() {
        assert!(KdfParams::DEFAULT.validate().is_ok());
        assert!(KdfParams::RECOVERY.validate().is_ok());
        // Всё, что может выбрать калибровка, проходит проверку при чтении.
        let max = KdfParams {
            t: KdfParams::MAX_CALIBRATED_T,
            ..KdfParams::DEFAULT
        };
        assert!(max.validate().is_ok());
    }

    /// Замер для бюджета разблокировки (`docs/PLAN.md`: 0,5–1 с):
    /// `cargo test -p xonkypass-core --release -- --ignored --nocapture unlock_timing`
    #[test]
    #[ignore = "замер времени, запускается вручную"]
    fn unlock_timing() {
        let params = KdfParams::calibrate(UNLOCK_TARGET);
        let start = Instant::now();
        derive_key(b"password", &[0; 16], params).unwrap();
        let unlock = start.elapsed();
        let start = Instant::now();
        derive_key(b"recovery", &[0; 16], KdfParams::RECOVERY).unwrap();
        println!(
            "калибровка: {params:?}; разблокировка паролем: {unlock:?}; \
             ключом восстановления: {:?}",
            start.elapsed()
        );
    }
}
