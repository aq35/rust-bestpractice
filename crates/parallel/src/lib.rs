//! 並列化の実験対象。CPU バウンドな処理を「逐次 / std::thread::scope / rayon」で比べ、
//! 共有状態の扱い（Mutex / Atomic / スレッドローカル集計）と false sharing を測る。

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// 1 要素あたりのそれなりに重い CPU 処理（素数判定）
#[must_use]
pub fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    if n % 2 == 0 {
        return n == 2;
    }
    let mut d = 3;
    while d * d <= n {
        if n % d == 0 {
            return false;
        }
        d += 2;
    }
    true
}

#[must_use]
pub fn count_primes_seq(range: std::ops::Range<u64>) -> usize {
    range.filter(|&n| is_prime(n)).count()
}

/// 手動で等分割して scoped thread。仕事量が偏る（大きい数ほど重い）と遅いスレッドを待つ
#[must_use]
pub fn count_primes_scoped(range: std::ops::Range<u64>, threads: usize) -> usize {
    let len = range.end - range.start;
    let chunk = len.div_ceil(threads as u64);
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads as u64)
            .map(|t| {
                let lo = range.start + t * chunk;
                let hi = (lo + chunk).min(range.end);
                s.spawn(move || count_primes_seq(lo..hi))
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap_or(0)).sum()
    })
}

/// 交互割り当て（ストライド）で手動負荷分散
#[must_use]
pub fn count_primes_scoped_strided(range: std::ops::Range<u64>, threads: usize) -> usize {
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads as u64)
            .map(|t| {
                let r = range.clone();
                s.spawn(move || {
                    (r.start + t..r.end)
                        .step_by(threads)
                        .filter(|&n| is_prime(n))
                        .count()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap_or(0)).sum()
    })
}

/// rayon: work stealing で偏りを自動で吸収
#[must_use]
pub fn count_primes_rayon(range: std::ops::Range<u64>) -> usize {
    use rayon::prelude::*;
    range.into_par_iter().filter(|&n| is_prime(n)).count()
}

/// 要素が軽すぎる処理を並列化したとき（オーバーヘッドが勝つか）
#[must_use]
pub fn sum_rayon_tiny(v: &[u64]) -> u64 {
    use rayon::prelude::*;
    v.par_iter().copied().reduce(|| 0, u64::wrapping_add)
}

#[must_use]
pub fn sum_rayon_min_len(v: &[u64], min_len: usize) -> u64 {
    use rayon::prelude::*;
    v.par_iter()
        .with_min_len(min_len)
        .copied()
        .reduce(|| 0, u64::wrapping_add)
}

#[must_use]
pub fn sum_seq(v: &[u64]) -> u64 {
    v.iter().copied().fold(0, u64::wrapping_add)
}

// ---------- 共有カウンタ ----------

/// 全スレッドが 1 つの Mutex を取り合う
#[must_use]
pub fn counter_mutex(threads: usize, per_thread: u64) -> u64 {
    let m = Mutex::new(0u64);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                for _ in 0..per_thread {
                    *m.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
                }
            });
        }
    });
    m.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 1 つの Atomic を取り合う（ロックはないがキャッシュラインが行き来する）
#[must_use]
pub fn counter_atomic(threads: usize, per_thread: u64) -> u64 {
    let a = AtomicU64::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                for _ in 0..per_thread {
                    a.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });
    a.into_inner()
}

/// スレッドごとに別の Atomic。ただし隣接して並ぶので同じキャッシュラインに乗る（false sharing）
#[must_use]
pub fn counter_false_sharing(threads: usize, per_thread: u64) -> u64 {
    let slots: Vec<AtomicU64> = (0..threads).map(|_| AtomicU64::new(0)).collect();
    std::thread::scope(|s| {
        for slot in &slots {
            s.spawn(move || {
                for _ in 0..per_thread {
                    slot.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });
    slots.iter().map(|a| a.load(Ordering::Relaxed)).sum()
}

/// 64 バイト境界に揃えて別キャッシュラインに置く
#[repr(align(128))] // 隣接ラインのプリフェッチも避けるため 128
struct Padded(AtomicU64);

#[must_use]
pub fn counter_padded(threads: usize, per_thread: u64) -> u64 {
    let slots: Vec<Padded> = (0..threads).map(|_| Padded(AtomicU64::new(0))).collect();
    std::thread::scope(|s| {
        for slot in &slots {
            s.spawn(move || {
                for _ in 0..per_thread {
                    slot.0.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });
    slots.iter().map(|a| a.0.load(Ordering::Relaxed)).sum()
}

/// ローカル変数で集計し、最後に 1 回だけ合算（推奨）
#[must_use]
pub fn counter_local_then_merge(threads: usize, per_thread: u64) -> u64 {
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(move || {
                    let mut c = 0u64;
                    for _ in 0..per_thread {
                        c = std::hint::black_box(c + 1);
                    }
                    c
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap_or(0)).sum()
    })
}

// ---------- チャネル ----------

/// std mpsc: 1 producer / 1 consumer
#[must_use]
pub fn channel_std(n: u64, bound: Option<usize>) -> u64 {
    std::thread::scope(|s| {
        if let Some(b) = bound {
            let (tx, rx) = std::sync::mpsc::sync_channel(b);
            s.spawn(move || {
                for i in 0..n {
                    let _ = tx.send(i);
                }
            });
            rx.iter().fold(0, u64::wrapping_add)
        } else {
            let (tx, rx) = std::sync::mpsc::channel();
            s.spawn(move || {
                for i in 0..n {
                    let _ = tx.send(i);
                }
            });
            rx.iter().fold(0, u64::wrapping_add)
        }
    })
}

/// 1 件ずつではなく Vec でまとめて送る
#[must_use]
pub fn channel_std_batched(n: u64, batch: usize) -> u64 {
    std::thread::scope(|s| {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<u64>>(16);
        s.spawn(move || {
            let mut buf = Vec::with_capacity(batch);
            for i in 0..n {
                buf.push(i);
                if buf.len() == batch {
                    let _ = tx.send(std::mem::replace(&mut buf, Vec::with_capacity(batch)));
                }
            }
            if !buf.is_empty() {
                let _ = tx.send(buf);
            }
        });
        rx.iter().flatten().fold(0, u64::wrapping_add)
    })
}

#[must_use]
pub fn channel_crossbeam(n: u64, bound: usize) -> u64 {
    std::thread::scope(|s| {
        let (tx, rx) = crossbeam_channel::bounded(bound);
        s.spawn(move || {
            for i in 0..n {
                let _ = tx.send(i);
            }
        });
        rx.iter().fold(0, u64::wrapping_add)
    })
}

// ---------- tokio と CPU 処理 ----------

/// CPU 処理を async タスク内で直接実行（ワーカースレッドを塞ぐ）しつつ、
/// 同時に 1ms 間隔の軽いタスク（= 他のリクエスト）の遅延を測る。
/// 戻り値: 軽いタスクの最大遅延
pub async fn tick_latency_under_cpu_load(mode: CpuMode, jobs: usize) -> std::time::Duration {
    use std::time::{Duration, Instant};
    let ticker = tokio::spawn(async {
        let mut worst = Duration::ZERO;
        for _ in 0..50 {
            let t = Instant::now();
            tokio::time::sleep(Duration::from_millis(1)).await;
            let lag = t.elapsed().saturating_sub(Duration::from_millis(1));
            worst = worst.max(lag);
        }
        worst
    });
    tokio::task::yield_now().await;
    let mut hs = Vec::new();
    for j in 0..jobs {
        let lo = 5_000_000 + j as u64 * 200_000;
        let range = lo..lo + 200_000;
        hs.push(match mode {
            CpuMode::Inline => tokio::spawn(async move { count_primes_seq(range) }),
            CpuMode::SpawnBlocking => {
                tokio::spawn(async move {
                    tokio::task::spawn_blocking(move || count_primes_seq(range))
                        .await
                        .unwrap_or(0)
                })
            }
            CpuMode::Rayon => tokio::spawn(async move {
                let (tx, rx) = tokio::sync::oneshot::channel();
                rayon::spawn(move || {
                    let _ = tx.send(count_primes_seq(range));
                });
                rx.await.unwrap_or(0)
            }),
        });
    }
    for h in hs {
        let _ = h.await;
    }
    ticker.await.unwrap_or_default()
}

#[derive(Clone, Copy, Debug)]
pub enum CpuMode {
    Inline,
    SpawnBlocking,
    Rayon,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prime_variants_agree() {
        let r = 0..50_000;
        let e = count_primes_seq(r.clone());
        assert_eq!(e, 5133);
        assert_eq!(e, count_primes_scoped(r.clone(), 4));
        assert_eq!(e, count_primes_scoped_strided(r.clone(), 4));
        assert_eq!(e, count_primes_rayon(r));
    }

    #[test]
    fn counters_agree() {
        for f in [
            counter_mutex,
            counter_atomic,
            counter_false_sharing,
            counter_padded,
            counter_local_then_merge,
        ] {
            assert_eq!(f(4, 1000), 4000);
        }
    }

    #[test]
    fn channels_agree() {
        let e: u64 = (0..10_000).sum();
        assert_eq!(channel_std(10_000, None), e);
        assert_eq!(channel_std(10_000, Some(64)), e);
        assert_eq!(channel_std_batched(10_000, 256), e);
        assert_eq!(channel_crossbeam(10_000, 64), e);
    }

    #[test]
    fn sums_agree() {
        let v: Vec<u64> = (0..100_000).collect();
        assert_eq!(sum_seq(&v), sum_rayon_tiny(&v));
        assert_eq!(sum_seq(&v), sum_rayon_min_len(&v, 4096));
    }
}
