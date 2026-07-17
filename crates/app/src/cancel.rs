//! Кооперативная отмена фоновых циклов.
//!
//! [`CancelFlag`] — общий примитив отмены для всех планировщиков приложения:
//! исходно был локальным для [`crate::history`] (загрузчик истории, T10,
//! фаза 11.3), отсюда его используют и планировщики поллинга —
//! [`crate::ingest::IngestService::run`] и
//! [`crate::algo_ingest::AlgoIngestService::run`] — которые раньше крутили
//! `loop { ticker.tick().await; ... }` без какого-либо способа остановиться,
//! кроме убийства процесса. Флаг — простой `Arc<AtomicBool>`, а не полноценный
//! `tokio_util::sync::CancellationToken` (лишняя зависимость ради того же
//! эффекта): цикл поллинга и так просыпается по таймеру `interval`, поэтому
//! достаточно проверять флаг на каждом пробуждении — быстрее реагировать
//! незачем (следующий такт всё равно не раньше `interval`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Кооперативный флаг отмены. Клонируется дёшево (общий `Arc<AtomicBool>`):
/// одна копия остаётся у владельца задачи (реестра/вызывающей стороны),
/// другую проверяет сам цикл.
#[derive(Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    /// Новый неотменённый флаг.
    pub fn new() -> Self {
        Self::default()
    }

    /// Пометить цикл отменённым (идемпотентно).
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Отменён ли цикл.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_flag_is_not_cancelled() {
        assert!(!CancelFlag::new().is_cancelled());
    }

    #[test]
    fn cancel_is_visible_through_clones() {
        let flag = CancelFlag::new();
        let clone = flag.clone();
        clone.cancel();
        assert!(flag.is_cancelled(), "отмена должна быть видна через клон");
    }

    #[test]
    fn cancel_is_idempotent() {
        let flag = CancelFlag::new();
        flag.cancel();
        flag.cancel();
        assert!(flag.is_cancelled());
    }
}
