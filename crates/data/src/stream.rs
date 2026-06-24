//! Переподключаемые стримы (§ 0.3).
//!
//! Стрим Finam обрывается ~раз в 24 ч, плюс возможны сетевые сбои. [`reconnecting`]
//! оборачивает «фабрику стрима» и прозрачно переподключается с экспоненциальным
//! backoff, пробрасывая элементы наружу. На успешном подключении счётчик попыток
//! сбрасывается; ошибки элемента/подключения пробрасываются и инициируют
//! переподключение.

use std::future::Future;

use tokio_stream::{Stream, StreamExt};

use crate::resilience::backoff_delay;
use crate::DataError;

/// Построить бесконечный переподключаемый стрим из фабрики `connect`.
///
/// `connect(attempt)` создаёт новый исходный стрим (или ошибку подключения).
/// Полученные элементы пробрасываются; когда исходный стрим завершается или
/// отдаёт ошибку, выполняется пауза `backoff_delay` и переподключение.
///
/// Стрим бесконечный: потребитель ограничивает его сам (`take`, drop).
pub fn reconnecting<T, S, F, Fut>(connect: F) -> impl Stream<Item = Result<T, DataError>>
where
    F: Fn(u32) -> Fut,
    Fut: Future<Output = Result<S, DataError>>,
    S: Stream<Item = Result<T, DataError>>,
{
    async_stream::stream! {
        let mut attempt = 0u32;
        loop {
            match connect(attempt).await {
                Ok(inner) => {
                    attempt = 0; // успешное подключение — сбрасываем backoff
                    tokio::pin!(inner);
                    while let Some(item) = inner.next().await {
                        let is_err = item.is_err();
                        yield item;
                        if is_err {
                            break; // после ошибки элемента — переподключаемся
                        }
                    }
                    // исходный стрим завершился (например, обрыв ~24 ч)
                }
                Err(e) => {
                    yield Err(e);
                }
            }

            tokio::time::sleep(backoff_delay(attempt)).await;
            attempt = attempt.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn reconnects_after_each_stream_ends() {
        // Каждое подключение отдаёт один элемент (свой номер) и завершается.
        let connects = Arc::new(AtomicU32::new(0));
        let counter = connects.clone();
        let s = reconnecting(move |_attempt| {
            let n = counter.fetch_add(1, Ordering::SeqCst);
            async move { Ok::<_, DataError>(tokio_stream::iter([Ok::<u32, DataError>(n)])) }
        });
        tokio::pin!(s);

        let mut got = Vec::new();
        for _ in 0..3 {
            got.push(s.next().await.unwrap().unwrap());
        }
        // Три элемента — значит было три переподключения.
        assert_eq!(got, vec![0, 1, 2]);
        assert!(connects.load(Ordering::SeqCst) >= 3);
    }

    #[tokio::test(start_paused = true)]
    async fn surfaces_connect_error_then_reconnects() {
        let calls = Arc::new(AtomicU32::new(0));
        let counter = calls.clone();
        let s = reconnecting(move |_attempt| {
            let n = counter.fetch_add(1, Ordering::SeqCst);
            async move {
                if n == 0 {
                    Err(DataError::Transport("обрыв".into()))
                } else {
                    Ok(tokio_stream::iter([Ok::<u32, DataError>(42)]))
                }
            }
        });
        tokio::pin!(s);

        // Первое подключение — ошибка, она пробрасывается.
        assert!(s.next().await.unwrap().is_err());
        // После backoff — успешное переподключение и элемент.
        assert_eq!(s.next().await.unwrap().unwrap(), 42);
    }
}
