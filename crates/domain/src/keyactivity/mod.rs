//! Движок «Key Activity» (ключевые активности): типизированная модель правил,
//! чистый интерпретатор и встроенный набор правил по умолчанию (фаза 10).
//!
//! Правило — это булева композиция условий над рыночными метриками
//! (объём/дисбаланс/OI/HI2/спред/изменение цены), ограниченная областью
//! (тикер | набор тикеров | весь рынок | класс актива). Композиция поддерживает
//! `AND`/`OR`/`NOT` и паттерн «если A то B». Вся модель сериализуема в JSON
//! (хранение пользовательских настроек) и интерпретируется чисто, без сети.
//!
//! Сборка промпта для LLM-итога — в подмодуле [`prompt`] (задача 10.4.2).

pub mod prompt;

use serde::{Deserialize, Serialize};

use crate::model::AssetClass;

/// Рыночная метрика, по которой строится условие.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    /// Объём за период.
    Volume,
    /// z-score объёма (аномальность).
    VolumeZScore,
    /// Дисбаланс потока (−1..1).
    Disbalance,
    /// Изменение открытого интереса.
    OiChange,
    /// Индекс концентрации HI2.
    Hi2,
    /// Относительный спред BBO.
    Spread,
    /// Изменение цены за период (доля).
    PriceChange,
}

impl Metric {
    /// Человекочитаемая подпись.
    pub fn label(self) -> &'static str {
        match self {
            Metric::Volume => "объём",
            Metric::VolumeZScore => "z-score объёма",
            Metric::Disbalance => "дисбаланс",
            Metric::OiChange => "изменение OI",
            Metric::Hi2 => "концентрация HI2",
            Metric::Spread => "спред",
            Metric::PriceChange => "изменение цены",
        }
    }
}

/// Оператор сравнения метрики с порогом.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparator {
    Gt,
    Lt,
    Ge,
    Le,
    /// |metric| ≥ threshold (модуль — для дисбаланса/изменений в обе стороны).
    AbsGe,
}

impl Comparator {
    fn holds(self, value: f64, threshold: f64) -> bool {
        match self {
            Comparator::Gt => value > threshold,
            Comparator::Lt => value < threshold,
            Comparator::Ge => value >= threshold,
            Comparator::Le => value <= threshold,
            Comparator::AbsGe => value.abs() >= threshold,
        }
    }
}

/// Элементарное условие: метрика `cmp` порог.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    pub metric: Metric,
    pub cmp: Comparator,
    pub threshold: f64,
}

impl Condition {
    pub fn new(metric: Metric, cmp: Comparator, threshold: f64) -> Self {
        Condition {
            metric,
            cmp,
            threshold,
        }
    }
}

/// Булева композиция условий.
///
/// `IfThen(a, b)` — паттерн «если A то B»: триггерится, когда выполнены **и**
/// антецедент `A`, **и** консеквент `B` (антецедент гейтирует консеквент). Это
/// семантика триггера, а не логической импликации `¬A∨B` (которая срабатывала
/// бы при ложном `A`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Cond(Condition),
    And(Vec<Expr>),
    Or(Vec<Expr>),
    Not(Box<Expr>),
    IfThen(Box<Expr>, Box<Expr>),
}

impl Expr {
    /// Удобный конструктор условия.
    pub fn cond(metric: Metric, cmp: Comparator, threshold: f64) -> Expr {
        Expr::Cond(Condition::new(metric, cmp, threshold))
    }

    /// Вычислить предикат на образце метрик.
    pub fn eval(&self, s: &Sample) -> bool {
        match self {
            Expr::Cond(c) => c.cmp.holds(s.metric(c.metric), c.threshold),
            Expr::And(es) => es.iter().all(|e| e.eval(s)),
            Expr::Or(es) => es.iter().any(|e| e.eval(s)),
            Expr::Not(e) => !e.eval(s),
            Expr::IfThen(a, b) => a.eval(s) && b.eval(s),
        }
    }

    /// Первичная метрика выражения (для отображения в строке результата):
    /// первая встреченная при обходе в глубину.
    pub fn primary_metric(&self) -> Option<Metric> {
        match self {
            Expr::Cond(c) => Some(c.metric),
            Expr::And(es) | Expr::Or(es) => es.iter().find_map(|e| e.primary_metric()),
            Expr::Not(e) => e.primary_metric(),
            Expr::IfThen(a, b) => a.primary_metric().or_else(|| b.primary_metric()),
        }
    }
}

/// Область применения правила.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum Scope {
    /// Один тикер.
    Ticker(String),
    /// Набор тикеров.
    Tickers(Vec<String>),
    /// Весь рынок (любой инструмент).
    Market,
    /// Класс актива.
    AssetClass(AssetClass),
}

impl Scope {
    /// Подпадает ли образец под область.
    pub fn matches(&self, s: &Sample) -> bool {
        match self {
            Scope::Ticker(t) => s.secid == *t,
            Scope::Tickers(ts) => ts.contains(&s.secid),
            Scope::Market => true,
            Scope::AssetClass(c) => s.asset_class == Some(*c),
        }
    }
}

/// Правило Key Activity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub scope: Scope,
    pub expr: Expr,
    /// Вес/важность (приоритет в выдаче).
    pub weight: f64,
}

/// Образец метрик инструмента за период (вход интерпретатора).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub secid: String,
    #[serde(default)]
    pub asset_class: Option<AssetClass>,
    pub ts: i64,
    #[serde(default)]
    pub volume: f64,
    #[serde(default)]
    pub volume_z: f64,
    #[serde(default)]
    pub disb: f64,
    #[serde(default)]
    pub oi_change: f64,
    #[serde(default)]
    pub hi2: f64,
    #[serde(default)]
    pub spread: f64,
    #[serde(default)]
    pub price_change: f64,
}

impl Sample {
    /// Значение метрики у образца.
    pub fn metric(&self, m: Metric) -> f64 {
        match m {
            Metric::Volume => self.volume,
            Metric::VolumeZScore => self.volume_z,
            Metric::Disbalance => self.disb,
            Metric::OiChange => self.oi_change,
            Metric::Hi2 => self.hi2,
            Metric::Spread => self.spread,
            Metric::PriceChange => self.price_change,
        }
    }
}

/// Период анализа.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Period {
    /// Час — период по умолчанию (задача 10.3.4).
    #[default]
    H1,
    D1,
    W1,
    M1,
    M3,
}

impl Period {
    /// Длительность периода в секундах (месяц ≈ 30 дней).
    pub fn seconds(self) -> i64 {
        match self {
            Period::H1 => 3_600,
            Period::D1 => 86_400,
            Period::W1 => 604_800,
            Period::M1 => 2_592_000,
            Period::M3 => 7_776_000,
        }
    }

    /// Подпись периода.
    pub fn label(self) -> &'static str {
        match self {
            Period::H1 => "1h",
            Period::D1 => "1d",
            Period::W1 => "1w",
            Period::M1 => "1m",
            Period::M3 => "3m",
        }
    }

    /// Диапазон `(from, till)` относительно момента `now` (UNIX-секунды).
    pub fn resolve(self, now: i64) -> (i64, i64) {
        (now - self.seconds(), now)
    }

    /// Разбор подписи периода (`1h`/`1d`/`1w`/`1m`/`3m`).
    pub fn from_label(s: &str) -> Option<Period> {
        Some(match s {
            "1h" => Period::H1,
            "1d" => Period::D1,
            "1w" => Period::W1,
            "1m" => Period::M1,
            "3m" => Period::M3,
            _ => return None,
        })
    }
}

/// Строка результата «Key activity».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyActivityRow {
    pub secid: String,
    pub rule_id: String,
    pub rule_name: String,
    /// Первичная метрика сработавшего правила.
    pub metric: Metric,
    /// Значение метрики у образца.
    pub value: f64,
    pub ts: i64,
    /// Важность (вес правила).
    pub importance: f64,
}

/// Прогнать правила по образцам: для каждого образца, подпадающего под область
/// правила и удовлетворяющего выражению, формируется строка результата.
///
/// Результат отсортирован по убыванию важности, затем по тикеру (стабильно).
pub fn evaluate(rules: &[Rule], samples: &[Sample]) -> Vec<KeyActivityRow> {
    let mut rows = Vec::new();
    for rule in rules {
        for s in samples {
            if !rule.scope.matches(s) {
                continue;
            }
            if rule.expr.eval(s) {
                let metric = rule.expr.primary_metric().unwrap_or(Metric::Volume);
                rows.push(KeyActivityRow {
                    secid: s.secid.clone(),
                    rule_id: rule.id.clone(),
                    rule_name: rule.name.clone(),
                    metric,
                    value: s.metric(metric),
                    ts: s.ts,
                    importance: rule.weight,
                });
            }
        }
    }
    rows.sort_by(|a, b| {
        b.importance
            .partial_cmp(&a.importance)
            .unwrap()
            .then_with(|| a.secid.cmp(&b.secid))
    });
    rows
}

/// Встроенный набор правил по умолчанию (область — весь рынок).
///
/// Каждое правило задокументировано назначением; пороги — рабочие значения,
/// настраиваемые в UI (задача 10.8.2):
/// - `top_volume` — высокий оборот (топ по обороту);
/// - `anomalous_volume` — аномальный объём (z-score ≥ 3);
/// - `disbalance_reversal` — резкий разворот дисбаланса (|disb| ≥ 0.6);
/// - `futoi_extreme` — экстремум FUTOI (|ΔOI| ≥ 1000);
/// - `hi2_spike` — всплеск концентрации (HI2 ≥ 0.4);
/// - `mega_move` — композит: аномальный объём **и** перевес потока
///   (паттерн «если объём то дисбаланс»).
pub fn default_rules() -> Vec<Rule> {
    vec![
        Rule {
            id: "top_volume".into(),
            name: "Высокий оборот".into(),
            scope: Scope::Market,
            expr: Expr::cond(Metric::Volume, Comparator::Ge, 1_000_000.0),
            weight: 1.0,
        },
        Rule {
            id: "anomalous_volume".into(),
            name: "Аномальный объём".into(),
            scope: Scope::Market,
            expr: Expr::cond(Metric::VolumeZScore, Comparator::Ge, 3.0),
            weight: 3.0,
        },
        Rule {
            id: "disbalance_reversal".into(),
            name: "Разворот дисбаланса".into(),
            scope: Scope::Market,
            expr: Expr::cond(Metric::Disbalance, Comparator::AbsGe, 0.6),
            weight: 2.0,
        },
        Rule {
            id: "futoi_extreme".into(),
            name: "Экстремум FUTOI".into(),
            scope: Scope::Market,
            expr: Expr::cond(Metric::OiChange, Comparator::AbsGe, 1_000.0),
            weight: 2.5,
        },
        Rule {
            id: "hi2_spike".into(),
            name: "Всплеск концентрации".into(),
            scope: Scope::Market,
            expr: Expr::cond(Metric::Hi2, Comparator::Ge, 0.4),
            weight: 2.0,
        },
        Rule {
            id: "mega_move".into(),
            name: "Мега-движение".into(),
            scope: Scope::Market,
            expr: Expr::IfThen(
                Box::new(Expr::cond(Metric::VolumeZScore, Comparator::Ge, 3.0)),
                Box::new(Expr::cond(Metric::Disbalance, Comparator::AbsGe, 0.5)),
            ),
            weight: 4.0,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(secid: &str) -> Sample {
        Sample {
            secid: secid.into(),
            asset_class: Some(AssetClass::Equity),
            ts: 1000,
            volume: 0.0,
            volume_z: 0.0,
            disb: 0.0,
            oi_change: 0.0,
            hi2: 0.0,
            spread: 0.0,
            price_change: 0.0,
        }
    }

    #[test]
    fn comparator_semantics() {
        assert!(Comparator::Gt.holds(2.0, 1.0));
        assert!(!Comparator::Gt.holds(1.0, 1.0));
        assert!(Comparator::Ge.holds(1.0, 1.0));
        assert!(Comparator::AbsGe.holds(-0.7, 0.6));
        assert!(!Comparator::AbsGe.holds(-0.5, 0.6));
    }

    #[test]
    fn and_or_not_compose() {
        let mut s = sample("X");
        s.volume_z = 4.0;
        s.disb = 0.1;
        let expr = Expr::And(vec![
            Expr::cond(Metric::VolumeZScore, Comparator::Ge, 3.0),
            Expr::Not(Box::new(Expr::cond(
                Metric::Disbalance,
                Comparator::AbsGe,
                0.5,
            ))),
        ]);
        assert!(expr.eval(&s));
        s.disb = 0.8;
        assert!(!expr.eval(&s));
    }

    #[test]
    fn if_then_requires_both() {
        let expr = Expr::IfThen(
            Box::new(Expr::cond(Metric::VolumeZScore, Comparator::Ge, 3.0)),
            Box::new(Expr::cond(Metric::Disbalance, Comparator::AbsGe, 0.5)),
        );
        let mut s = sample("X");
        // Антецедент ложен → не триггерит (в отличие от логической импликации).
        s.volume_z = 1.0;
        s.disb = 0.9;
        assert!(!expr.eval(&s));
        // Оба истинны → триггерит.
        s.volume_z = 4.0;
        assert!(expr.eval(&s));
    }

    #[test]
    fn scope_filters() {
        let s = sample("SBER");
        assert!(Scope::Market.matches(&s));
        assert!(Scope::Ticker("SBER".into()).matches(&s));
        assert!(!Scope::Ticker("GAZP".into()).matches(&s));
        assert!(Scope::Tickers(vec!["A".into(), "SBER".into()]).matches(&s));
        assert!(Scope::AssetClass(AssetClass::Equity).matches(&s));
        assert!(!Scope::AssetClass(AssetClass::Bond).matches(&s));
    }

    #[test]
    fn period_resolves_and_roundtrips() {
        assert_eq!(Period::default(), Period::H1);
        assert_eq!(Period::H1.resolve(10_000), (10_000 - 3600, 10_000));
        assert_eq!(Period::from_label("3m"), Some(Period::M3));
        assert_eq!(Period::from_label("x"), None);
        assert_eq!(Period::W1.label(), "1w");
    }

    #[test]
    fn evaluate_sorts_by_importance() {
        let rules = default_rules();
        let mut hot = sample("SBER");
        hot.volume_z = 5.0;
        hot.disb = 0.8;
        hot.volume = 2_000_000.0;
        let mut quiet = sample("GAZP");
        quiet.volume = 500.0;
        let rows = evaluate(&rules, &[hot, quiet]);
        // SBER срабатывает по нескольким правилам; первое — самое важное (mega_move).
        assert!(!rows.is_empty());
        assert_eq!(rows[0].secid, "SBER");
        assert_eq!(rows[0].rule_id, "mega_move");
        // Убедимся, что важности невозрастающие.
        for w in rows.windows(2) {
            assert!(w[0].importance >= w[1].importance);
        }
    }

    #[test]
    fn rule_serializes_to_json_roundtrip() {
        let rules = default_rules();
        let json = serde_json::to_string(&rules).unwrap();
        let back: Vec<Rule> = serde_json::from_str(&json).unwrap();
        assert_eq!(rules, back);
    }

    #[test]
    fn empty_samples_yield_no_rows() {
        assert!(evaluate(&default_rules(), &[]).is_empty());
    }
}
