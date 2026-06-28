//! Лёгкий парсер `.env` без внешних зависимостей.
//!
//! Боевой секрет Finam удобно держать в `.env` (он в `.gitignore`, в репозиторий
//! не попадает). Этот модуль читает такой файл и достаёт API-секрет, принимая
//! несколько распространённых имён ключа (`FINAM_API_SECRET`, `FINAM_SECRET`,
//! `Finam_SECRET`) — без чувствительности к регистру.
//!
//! Парсинг — чистая функция над текстом ([`parse`]): не трогает файловую систему
//! и окружение, поэтому тестируется кросс-платформенно. Поверх неё —
//! ввод-вывод: [`secret_from_dotenv_file`]/[`find_dotenv_secret`].

use std::path::{Path, PathBuf};

/// Имена ключа API-секрета Finam, которые распознаём в `.env`/окружении
/// (сравнение без учёта регистра).
pub const SECRET_KEYS: &[&str] = &["FINAM_API_SECRET", "FINAM_SECRET"];

/// Имя переменной окружения, которую ожидает остальной код.
pub const ENV_VAR: &str = "FINAM_API_SECRET";

/// Распарсить содержимое `.env` в пары «ключ → значение».
///
/// Поддерживает: пустые строки и комментарии (`#`), необязательный префикс
/// `export`, значения в одинарных/двойных кавычках и завершающие комментарии
/// после некавыченного значения. Пробелы вокруг ключа и значения срезаются.
pub fn parse(contents: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for raw in contents.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").map_or(line, str::trim);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        out.push((key.to_owned(), unquote(value.trim())));
    }
    out
}

/// Снять кавычки со значения или, для некавыченного, отрезать завершающий
/// `#`-комментарий.
fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if value.len() >= 2 {
        let first = bytes[0];
        if (first == b'"' || first == b'\'') && bytes[bytes.len() - 1] == first {
            return value[1..value.len() - 1].to_owned();
        }
    }
    // Некавыченное значение: всё после ` #` — комментарий.
    match value.split_once(" #") {
        Some((head, _)) => head.trim_end().to_owned(),
        None => value.to_owned(),
    }
}

/// Достать секрет Finam из уже распарсенных пар (по [`SECRET_KEYS`],
/// регистронезависимо). Первое непустое совпадение выигрывает.
pub fn secret_from_pairs(pairs: &[(String, String)]) -> Option<String> {
    for (key, value) in pairs {
        if SECRET_KEYS.iter().any(|k| k.eq_ignore_ascii_case(key)) {
            let v = value.trim();
            if !v.is_empty() {
                return Some(v.to_owned());
            }
        }
    }
    None
}

/// Прочитать `.env` по пути и вернуть секрет Finam, если он там есть.
/// Отсутствие файла — не ошибка (вернёт `None`).
pub fn secret_from_dotenv_file(path: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;
    secret_from_pairs(&parse(&contents))
}

/// Найти `.env` начиная с `start` и поднимаясь вверх по дереву каталогов,
/// вернуть первый найденный секрет Finam. Поиск ограничен `max_depth` уровнями.
pub fn find_dotenv_secret(start: &Path, max_depth: usize) -> Option<String> {
    let mut dir: Option<PathBuf> = Some(start.to_path_buf());
    for _ in 0..=max_depth {
        let Some(current) = dir else { break };
        let candidate = current.join(".env");
        if let Some(secret) = secret_from_dotenv_file(&candidate) {
            return Some(secret);
        }
        dir = current.parent().map(Path::to_path_buf);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_pairs() {
        let pairs = parse("A=1\nB=two\n");
        assert_eq!(
            pairs,
            vec![("A".into(), "1".into()), ("B".into(), "two".into())]
        );
    }

    #[test]
    fn skips_comments_and_blanks() {
        let pairs = parse("# comment\n\n  \nKEY=val\n");
        assert_eq!(pairs, vec![("KEY".into(), "val".into())]);
    }

    #[test]
    fn handles_export_prefix() {
        let pairs = parse("export FOO=bar\n");
        assert_eq!(pairs, vec![("FOO".into(), "bar".into())]);
    }

    #[test]
    fn strips_quotes() {
        assert_eq!(parse("A=\"q v\"\n"), vec![("A".into(), "q v".into())]);
        assert_eq!(parse("B='s v'\n"), vec![("B".into(), "s v".into())]);
    }

    #[test]
    fn strips_trailing_comment_on_unquoted() {
        assert_eq!(parse("A=val # note\n"), vec![("A".into(), "val".into())]);
        // Решётка без ведущего пробела — часть значения (не комментарий).
        assert_eq!(parse("A=va#l\n"), vec![("A".into(), "va#l".into())]);
    }

    #[test]
    fn finds_secret_by_known_keys_case_insensitive() {
        let pairs = parse("Finam_SECRET=tapi_sk_abc\n");
        assert_eq!(secret_from_pairs(&pairs).as_deref(), Some("tapi_sk_abc"));

        let pairs = parse("FINAM_API_SECRET=tapi_sk_xyz\n");
        assert_eq!(secret_from_pairs(&pairs).as_deref(), Some("tapi_sk_xyz"));
    }

    #[test]
    fn ignores_empty_secret_value() {
        let pairs = parse("FINAM_SECRET=\nFINAM_API_SECRET=real\n");
        assert_eq!(secret_from_pairs(&pairs).as_deref(), Some("real"));
    }

    #[test]
    fn no_secret_returns_none() {
        let pairs = parse("OTHER=1\n");
        assert_eq!(secret_from_pairs(&pairs), None);
    }

    #[test]
    fn reads_secret_from_file_and_walks_up() {
        let base = std::env::temp_dir().join(format!("mf-dotenv-{}", std::process::id()));
        let nested = base.join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(base.join(".env"), "Finam_SECRET=tapi_sk_walk\n").unwrap();

        // Прямо в каталоге секрета — нет файла, поднимаемся вверх и находим.
        assert_eq!(
            find_dotenv_secret(&nested, 4).as_deref(),
            Some("tapi_sk_walk")
        );
        // Несуществующий путь — None, не паника.
        assert_eq!(secret_from_dotenv_file(&base.join("missing.env")), None);

        std::fs::remove_dir_all(&base).ok();
    }
}
