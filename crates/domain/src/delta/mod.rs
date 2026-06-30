//! Анализ потока ордеров (order flow): дельта/footprint и детектирующие роботы.
//!
//! - [`footprint`] — раскладка объёма бара по ценам и сторонам агрессора,
//!   дельта бара и накопленная дельта (CVD);
//! - [`robots`] — распознавание следов известных алгоритмов на ленте
//!   (равные лоты, айсберги, поглощение) для оверлея на графике.
//!
//! Только анализ и визуализация — роботы не торгуют. Всё чисто и тестируемо.

pub mod footprint;
pub mod robots;

pub use footprint::{footprint, FootprintBar, FootprintCell};
pub use robots::{
    detect_absorption, detect_icebergs, detect_same_lots, RobotConfig, RobotKind, RobotScanner,
    RobotSignal,
};
