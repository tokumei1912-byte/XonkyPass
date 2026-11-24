//! Скорость поиска на 10 000 сгенерированных записей.
//!
//! ```text
//! cargo run --release -p xonkypass-core --example search_bench
//! ```
//!
//! Цель плана: поиск не дольше 5 мс.

use std::time::{Duration, Instant};

use xonkypass_core::search::SearchIndex;
use xonkypass_core::{ItemMeta, ItemSummary, LoginMeta, NoteMeta, SearchOptions};

const ITEMS: usize = 10_000;
const RUNS: usize = 200;
const BUDGET: Duration = Duration::from_millis(5);

const SERVICES: &[&str] = &[
    "GitHub",
    "GitLab",
    "Bitbucket",
    "Google",
    "Яндекс",
    "Mail.ru",
    "ВКонтакте",
    "Telegram",
    "WhatsApp",
    "Discord",
    "Slack",
    "Notion",
    "Figma",
    "Trello",
    "Jira",
    "Confluence",
    "Steam",
    "Epic Games",
    "Battle.net",
    "Origin",
    "Ubisoft",
    "Netflix",
    "Кинопоиск",
    "Okko",
    "Spotify",
    "Apple ID",
    "Microsoft",
    "Outlook",
    "Skype",
    "Zoom",
    "Dropbox",
    "OneDrive",
    "Госуслуги",
    "Сбербанк",
    "Тинькофф",
    "Альфа-Банк",
    "ВТБ",
    "Озон",
    "Wildberries",
    "AliExpress",
    "Amazon",
    "eBay",
    "Avito",
    "ЦИАН",
    "hh.ru",
    "LinkedIn",
    "Habr",
    "Stack Overflow",
    "Reddit",
    "Twitter",
    "Facebook",
    "Instagram",
    "TikTok",
    "Twitch",
    "YouTube",
    "DigitalOcean",
    "Hetzner",
    "AWS",
    "Cloudflare",
    "Namecheap",
    "Reg.ru",
    "Timeweb",
    "Docker Hub",
    "npm",
    "crates.io",
    "PyPI",
    "JetBrains",
    "Postman",
    "Vercel",
    "Netlify",
    "Heroku",
    "Router",
    "NAS",
    "Wi-Fi",
    "1С",
];

const NAMES: &[&str] = &[
    "ivan", "petr", "anna", "maria", "alex", "olga", "dmitry", "elena", "sergey", "kate", "admin",
    "dev", "test", "user", "info", "support",
];

const DOMAINS: &[&str] = &[
    "gmail.com",
    "mail.ru",
    "yandex.ru",
    "outlook.com",
    "corp.ru",
];

const TAGS: &[&str] = &[
    "работа",
    "личное",
    "дом",
    "финансы",
    "игры",
    "dev",
    "work",
    "shopping",
    "соцсети",
    "почта",
    "облако",
    "серверы",
    "клиенты",
    "семья",
    "учёба",
    "здоровье",
    "путешествия",
    "старое",
    "2fa",
    "важное",
];

const WORDS: &[&str] = &[
    "пароль",
    "сервер",
    "доступ",
    "ключ",
    "адрес",
    "порт",
    "логин",
    "резервная",
    "копия",
    "договор",
    "счёт",
    "оплата",
    "телефон",
    "почта",
    "заметка",
    "список",
    "покупки",
    "проект",
    "задача",
    "встреча",
    "ssh",
    "root",
    "backup",
    "config",
    "token",
    "api",
    "server",
    "deploy",
    "staging",
    "prod",
    "database",
    "user",
    "secret",
    "note",
    "meeting",
    "todo",
    "invoice",
    "wifi",
    "router",
];

/// Детерминированный генератор: одинаковые данные при каждом запуске.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap()
    }

    fn pick<'a>(&mut self, list: &[&'a str]) -> &'a str {
        list[self.below(list.len())]
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
}

fn tags(rng: &mut Rng) -> Vec<String> {
    let mut tags: Vec<String> = (0..rng.below(4)).map(|_| rng.pick(TAGS).into()).collect();
    tags.dedup();
    tags
}

fn generate(rng: &mut Rng, i: usize, now: i64) -> ItemSummary {
    let service = rng.pick(SERVICES);
    let meta = if rng.chance(80) {
        let domain = service.to_lowercase().replace(' ', "");
        let domain = if domain.contains('.') {
            domain
        } else {
            format!("{domain}.com")
        };
        ItemMeta::Login(LoginMeta {
            title: if rng.chance(30) {
                format!("{service} {}", rng.below(10))
            } else {
                service.into()
            },
            username: format!(
                "{}{}@{}",
                rng.pick(NAMES),
                rng.below(100),
                rng.pick(DOMAINS)
            ),
            urls: (0..1 + rng.below(2))
                .map(|k| {
                    if k == 0 {
                        domain.clone()
                    } else {
                        format!("app.{domain}")
                    }
                })
                .collect(),
            apps: if rng.chance(10) {
                vec![format!("{}.exe", service.replace(' ', ""))]
            } else {
                vec![]
            },
            tags: tags(rng),
            favorite: rng.chance(5),
            has_totp: rng.chance(20),
        })
    } else {
        let len = 20 + rng.below(300);
        let body: Vec<&str> = (0..len).map(|_| rng.pick(WORDS)).collect();
        ItemMeta::Note(NoteMeta {
            title: format!("{} {}", rng.pick(WORDS), rng.pick(SERVICES)),
            body: body.join(" "),
            tags: tags(rng),
            favorite: rng.chance(5),
        })
    };
    let used = rng.chance(40);
    let day = 24 * 60 * 60 * 1000;
    ItemSummary {
        id: format!("01900000-0000-7000-8000-{i:012x}").parse().unwrap(),
        meta,
        created_at: now - 365 * day,
        updated_at: now - i64::try_from(rng.below(365)).unwrap() * day,
        deleted_at: None,
        revision: 1,
        use_count: if used {
            i64::try_from(rng.below(200)).unwrap()
        } else {
            0
        },
        last_used_at: used.then(|| now - i64::try_from(rng.below(90)).unwrap() * day),
    }
}

fn percentile(sorted: &[Duration], p: usize) -> Duration {
    sorted[(sorted.len() * p / 100).min(sorted.len() - 1)]
}

fn main() {
    let now = 1_800_000_000_000;
    let mut rng = Rng(0x5eed_1234_abcd_9876);
    let items: Vec<ItemSummary> = (0..ITEMS).map(|i| generate(&mut rng, i, now)).collect();

    let start = Instant::now();
    let index = SearchIndex::new(items);
    println!(
        "Индекс из {} записей: {:.1} мс, тегов: {}",
        index.len(),
        start.elapsed().as_secs_f64() * 1000.0,
        index.tags().len()
    );

    let options = SearchOptions {
        limit: 200,
        ..SearchOptions::default()
    };
    let queries = [
        "",
        "g",
        "git",
        "github",
        "гос",
        "ivan gmail",
        "#работа",
        "#работа git",
        "is:fav",
        "is:login has:totp",
        "is:login zzqqxx",
        "is:note zzqqxx",
        "сервер",
        "backup deploy",
        "zzqqxx",
        "пшерги",
    ];

    println!(
        "\n{:<20} {:>7} {:>9} {:>9} {:>9}",
        "запрос", "найдено", "p50, мс", "p95, мс", "max, мс"
    );
    let mut worst = Duration::ZERO;
    for query in queries {
        let mut times = Vec::with_capacity(RUNS);
        let mut total = 0;
        for _ in 0..RUNS {
            let start = Instant::now();
            let result = index.search(query, &options);
            times.push(start.elapsed());
            total = result.total;
            std::hint::black_box(result);
        }
        times.sort_unstable();
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let p95 = percentile(&times, 95);
        worst = worst.max(p95);
        println!(
            "{:<20} {:>7} {:>9.2} {:>9.2} {:>9.2}",
            format!("«{query}»"),
            total,
            ms(percentile(&times, 50)),
            ms(p95),
            ms(times[times.len() - 1])
        );
    }

    println!(
        "\nХудший p95: {:.2} мс — {}",
        worst.as_secs_f64() * 1000.0,
        if worst <= BUDGET {
            "укладывается в 5 мс"
        } else {
            "НЕ укладывается в 5 мс"
        }
    );
}
