//! Сборка промпта для LLM-итога по результатам Key Activity (задача 10.4.2).
//!
//! Чистая детерминированная функция: структурированный вход (строки активности
//! и контекст периода) превращается в текстовый промпт. Без сети. Жёсткое
//! ограничение длины (`max_chars`) с аккуратным усечением по строкам — провайдер
//! LLM вызывается отдельно в `data` за фичей `llm`.

use super::{KeyActivityRow, Period};

/// Собрать промпт для итогового резюме рынка.
///
/// Строки берутся в порядке убывания важности (как из [`super::evaluate`]) и
/// добавляются, пока не превышен `max_chars`; заголовок и инструкция всегда
/// сохраняются. Возвращаемый текст детерминирован при одинаковом входе.
pub fn build_prompt(rows: &[KeyActivityRow], period: Period, max_chars: usize) -> String {
    let header = format!(
        "Ты — рыночный аналитик. Кратко и по делу резюмируй ключевые активности \
         российского рынка за период {}. Сгруппируй по значимости, отметь \
         аномалии объёма, дисбалансы потока, экстремумы открытого интереса и \
         всплески концентрации. Не выдумывай данных сверх приведённых.\n\n\
         Ключевые активности (важность ↓):\n",
        period.label()
    );
    let footer = "\n\nДай резюме в 3–5 предложениях.";

    let mut out = String::with_capacity(max_chars.min(4096));
    out.push_str(&header);

    // Бюджет на строки активностей — остаток после заголовка и подвала.
    let budget = max_chars.saturating_sub(header.len() + footer.len());
    for row in rows {
        let line = format!(
            "- {secid}: {rule} ({metric} = {value:.4}, важность {imp:.1})\n",
            secid = row.secid,
            rule = row.rule_name,
            metric = row.metric.label(),
            value = row.value,
            imp = row.importance,
        );
        if out.len() + line.len() - header.len() > budget {
            out.push_str("- … (список усечён)\n");
            break;
        }
        out.push_str(&line);
    }

    out.push_str(footer);
    out
}

/// Локальный текстовый свод без LLM — грациозная деградация при отсутствии
/// ключа/сети (задача 10.4.3). Перечисляет топ-`limit` активностей.
pub fn fallback_summary(rows: &[KeyActivityRow], period: Period, limit: usize) -> String {
    if rows.is_empty() {
        return format!(
            "За период {} ключевых активностей не обнаружено.",
            period.label()
        );
    }
    let mut out = format!(
        "Ключевые активности за период {} (LLM недоступен, локальный свод):\n",
        period.label()
    );
    for row in rows.iter().take(limit) {
        out.push_str(&format!(
            "• {} — {} ({} = {:.4})\n",
            row.secid,
            row.rule_name,
            row.metric.label(),
            row.value
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::{KeyActivityRow, Metric, Period};
    use super::*;

    fn row(secid: &str, importance: f64) -> KeyActivityRow {
        KeyActivityRow {
            secid: secid.into(),
            rule_id: "r".into(),
            rule_name: "Аномальный объём".into(),
            metric: Metric::VolumeZScore,
            value: 4.2,
            ts: 1000,
            importance,
        }
    }

    #[test]
    fn prompt_contains_header_and_rows() {
        let rows = vec![row("SBER", 3.0), row("GAZP", 2.0)];
        let p = build_prompt(&rows, Period::H1, 10_000);
        assert!(p.contains("период 1h"));
        assert!(p.contains("SBER"));
        assert!(p.contains("GAZP"));
        assert!(p.ends_with("предложениях."));
    }

    #[test]
    fn prompt_is_deterministic() {
        let rows = vec![row("SBER", 3.0)];
        let a = build_prompt(&rows, Period::D1, 5_000);
        let b = build_prompt(&rows, Period::D1, 5_000);
        assert_eq!(a, b);
    }

    #[test]
    fn prompt_respects_char_budget() {
        let rows: Vec<KeyActivityRow> = (0..1000).map(|i| row(&format!("TICK{i}"), 1.0)).collect();
        let p = build_prompt(&rows, Period::M3, 600);
        // Усечение сработало и подвал на месте.
        assert!(p.contains("усечён"));
        assert!(p.ends_with("предложениях."));
    }

    #[test]
    fn fallback_lists_top_rows() {
        let rows = vec![row("SBER", 3.0), row("GAZP", 2.0), row("LKOH", 1.0)];
        let s = fallback_summary(&rows, Period::H1, 2);
        assert!(s.contains("SBER"));
        assert!(s.contains("GAZP"));
        assert!(!s.contains("LKOH")); // limit = 2
    }

    #[test]
    fn fallback_handles_empty() {
        let s = fallback_summary(&[], Period::W1, 5);
        assert!(s.contains("не обнаружено"));
    }
}
