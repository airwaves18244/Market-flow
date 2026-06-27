//! Воспроизведение истории (replay): курсор по упорядоченной шкале времени.
//!
//! Чистая логика без таймеров и async — владелец (планировщик в `app`/UI) сам
//! решает, когда сделать шаг и с какой скоростью. Курсор хранит отсортированные
//! метки времени кадров и позицию «сколько уже проиграно».

/// Курсор воспроизведения по возрастающей шкале меток времени (UNIX-сек).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayCursor {
    frames: Vec<i64>,
    /// Число проигранных кадров: `0..=frames.len()`.
    pos: usize,
}

impl ReplayCursor {
    /// Создать курсор; метки сортируются по возрастанию, позиция — в начале.
    pub fn new(mut frames: Vec<i64>) -> Self {
        frames.sort_unstable();
        Self { frames, pos: 0 }
    }

    /// Число кадров.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Нет кадров.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Сколько кадров уже проиграно.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Достигнут ли конец воспроизведения.
    pub fn at_end(&self) -> bool {
        self.pos >= self.frames.len()
    }

    /// Метка последнего проигранного кадра (`None` до первого шага).
    pub fn current_ts(&self) -> Option<i64> {
        if self.pos == 0 {
            None
        } else {
            self.frames.get(self.pos - 1).copied()
        }
    }

    /// Прогресс `0..1` (пустой курсор считаем завершённым: `1.0`).
    pub fn progress(&self) -> f64 {
        if self.frames.is_empty() {
            1.0
        } else {
            self.pos as f64 / self.frames.len() as f64
        }
    }

    /// Шагнуть вперёд на один кадр; вернуть его метку (`None` в конце).
    pub fn step(&mut self) -> Option<i64> {
        if self.at_end() {
            return None;
        }
        let ts = self.frames[self.pos];
        self.pos += 1;
        Some(ts)
    }

    /// Перемотать так, чтобы проигранными считались все кадры с меткой `≤ ts`.
    pub fn seek(&mut self, ts: i64) {
        self.pos = self.frames.partition_point(|&f| f <= ts);
    }

    /// Вернуться в начало.
    pub fn reset(&mut self) {
        self.pos = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sorts_frames() {
        let c = ReplayCursor::new(vec![3, 1, 2]);
        assert_eq!(c.len(), 3);
        assert_eq!(c.pos(), 0);
        assert_eq!(c.current_ts(), None);
    }

    #[test]
    fn step_walks_frames_in_order() {
        let mut c = ReplayCursor::new(vec![30, 10, 20]);
        assert_eq!(c.step(), Some(10));
        assert_eq!(c.step(), Some(20));
        assert_eq!(c.current_ts(), Some(20));
        assert_eq!(c.step(), Some(30));
        assert!(c.at_end());
        assert_eq!(c.step(), None);
    }

    #[test]
    fn progress_tracks_position() {
        let mut c = ReplayCursor::new(vec![1, 2, 3, 4]);
        assert_eq!(c.progress(), 0.0);
        c.step();
        c.step();
        assert_eq!(c.progress(), 0.5);
    }

    #[test]
    fn seek_sets_position_to_inclusive_bound() {
        let mut c = ReplayCursor::new(vec![10, 20, 30, 40]);
        c.seek(25); // проиграны 10 и 20
        assert_eq!(c.pos(), 2);
        assert_eq!(c.current_ts(), Some(20));
        c.seek(30); // включительно
        assert_eq!(c.pos(), 3);
        assert_eq!(c.current_ts(), Some(30));
    }

    #[test]
    fn reset_returns_to_start() {
        let mut c = ReplayCursor::new(vec![1, 2, 3]);
        c.step();
        c.reset();
        assert_eq!(c.pos(), 0);
        assert_eq!(c.current_ts(), None);
    }

    #[test]
    fn empty_cursor_is_finished() {
        let mut c = ReplayCursor::new(vec![]);
        assert!(c.is_empty());
        assert!(c.at_end());
        assert_eq!(c.progress(), 1.0);
        assert_eq!(c.step(), None);
    }
}
