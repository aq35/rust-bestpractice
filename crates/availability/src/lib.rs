//! 可用性の実験。tokio の「止めた時計」（`start_paused`）で時間を仮想化し、
//! 障害（遅延・失敗・ハング・過負荷・パニック）を決定的に再現して、対策ごとの結果を数える。

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::time::{Instant, sleep, timeout};

/// 外部依存なしの決定的乱数（xorshift64*）
#[derive(Clone)]
pub struct Rng(u64);
impl Rng {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[must_use]
pub fn percentile(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let i = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[i]
}

// ============ 1. タイムアウトとリトライ ============

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CallError {
    #[error("upstream error")]
    Upstream,
    #[error("timed out")]
    Timeout,
}

/// 不安定な上流: 70% は 20ms で成功、20% は 20ms でエラー、10% は 30 秒ハング
pub async fn flaky_upstream(rng: &mut Rng) -> Result<(), CallError> {
    let r = rng.f64();
    if r < 0.70 {
        sleep(Duration::from_millis(20)).await;
        Ok(())
    } else if r < 0.90 {
        sleep(Duration::from_millis(20)).await;
        Err(CallError::Upstream)
    } else {
        sleep(Duration::from_secs(30)).await;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RetryPolicy {
    /// 何もしない
    Naive,
    /// タイムアウトのみ
    TimeoutOnly(Duration),
    /// タイムアウト + 指数バックオフ（ジッター付き）で最大 n 回
    TimeoutRetry { per_try: Duration, max_attempts: u32 },
    /// 上に加えて、全体の期限（deadline）を設ける
    TimeoutRetryDeadline { per_try: Duration, max_attempts: u32, deadline: Duration },
}

pub async fn call_with_policy(policy: RetryPolicy, rng: &mut Rng) -> Result<(), CallError> {
    match policy {
        RetryPolicy::Naive => flaky_upstream(rng).await,
        RetryPolicy::TimeoutOnly(t) => timeout(t, flaky_upstream(rng))
            .await
            .unwrap_or(Err(CallError::Timeout)),
        RetryPolicy::TimeoutRetry { per_try, max_attempts } => {
            retry(per_try, max_attempts, rng).await
        }
        RetryPolicy::TimeoutRetryDeadline { per_try, max_attempts, deadline } => {
            timeout(deadline, retry(per_try, max_attempts, rng))
                .await
                .unwrap_or(Err(CallError::Timeout))
        }
    }
}

async fn retry(per_try: Duration, max_attempts: u32, rng: &mut Rng) -> Result<(), CallError> {
    let mut backoff = Duration::from_millis(10);
    let mut last = Err(CallError::Timeout);
    for attempt in 0..max_attempts {
        last = timeout(per_try, flaky_upstream(rng))
            .await
            .unwrap_or(Err(CallError::Timeout));
        if last.is_ok() {
            return last;
        }
        if attempt + 1 < max_attempts {
            // full jitter: 0..backoff の一様乱数だけ待つ（同時リトライの集中を防ぐ）
            let jitter = backoff.mul_f64(rng.f64());
            sleep(jitter).await;
            backoff = (backoff * 2).min(Duration::from_secs(1));
        }
    }
    last
}

#[derive(Debug)]
pub struct Stats {
    pub ok: usize,
    pub total: usize,
    pub p50: Duration,
    pub p99: Duration,
    pub max: Duration,
}

pub async fn run_retry_experiment(policy: RetryPolicy, n: usize, seed: u64) -> Stats {
    let mut rng = Rng::new(seed);
    let mut lat = Vec::with_capacity(n);
    let mut ok = 0;
    for _ in 0..n {
        let t = Instant::now();
        if call_with_policy(policy, &mut rng).await.is_ok() {
            ok += 1;
        }
        lat.push(t.elapsed());
    }
    lat.sort();
    Stats {
        ok,
        total: n,
        p50: percentile(&lat, 0.5),
        p99: percentile(&lat, 0.99),
        max: *lat.last().unwrap_or(&Duration::ZERO),
    }
}

// ============ 2. 過負荷: 有界キュー / 同時実行制限 / 負荷制限 ============

#[derive(Clone, Copy, Debug)]
pub enum Admission {
    /// 来たものは全部受けて同時に処理（無制限）
    Unbounded,
    /// 同時実行数を制限し、超えた分は待たせる（待ち行列は無制限）
    SemaphoreQueue(usize),
    /// 同時実行数を制限し、空きがなければ即拒否（load shedding）
    SemaphoreShed(usize),
}

#[derive(Debug)]
pub struct OverloadStats {
    pub accepted: usize,
    pub rejected: usize,
    pub p50: Duration,
    pub p99: Duration,
    pub peak_in_flight: usize,
}

/// サーバーの処理容量は「同時 `capacity` 件まで 10ms、それを超えると全員が比例して遅くなる」
/// （CPU を分け合う状況の単純化）。到着は 1 秒あたり `rps` 件で `duration` の間続く。
/// tokio のタイマーは 1ms 単位なので `rps` は 1000 以下にする。
pub async fn run_overload(adm: Admission, rps: u64, duration: Duration, capacity: usize) -> OverloadStats {
    let in_flight = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let rejected = Arc::new(AtomicU64::new(0));
    let sem = Arc::new(tokio::sync::Semaphore::new(match adm {
        Admission::Unbounded => tokio::sync::Semaphore::MAX_PERMITS,
        Admission::SemaphoreQueue(n) | Admission::SemaphoreShed(n) => n,
    }));
    assert!(rps <= 1000, "timer resolution is 1ms");
    let interval = Duration::from_nanos(1_000_000_000 / rps);
    let total = (duration.as_nanos() / interval.as_nanos()) as usize;
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..total {
        let arrived = Instant::now();
        let (sem, in_flight, peak, rejected) =
            (sem.clone(), in_flight.clone(), peak.clone(), rejected.clone());
        set.spawn(async move {
            let _permit = match adm {
                Admission::SemaphoreShed(_) => match sem.try_acquire_owned() {
                    Ok(p) => p,
                    Err(_) => {
                        rejected.fetch_add(1, Ordering::Relaxed);
                        return None;
                    }
                },
                _ => sem.acquire_owned().await.ok()?,
            };
            let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            // 処理: 10ms 分の仕事を、現在の同時実行数で割った速度で進める
            let mut remaining = Duration::from_millis(10).as_secs_f64();
            let step = Duration::from_millis(1);
            while remaining > 0.0 {
                let share = (capacity as f64 / in_flight.load(Ordering::SeqCst) as f64).min(1.0);
                sleep(step).await;
                remaining -= step.as_secs_f64() * share;
            }
            in_flight.fetch_sub(1, Ordering::SeqCst);
            Some(arrived.elapsed())
        });
        sleep(interval).await;
    }
    let mut lat = Vec::new();
    while let Some(r) = set.join_next().await {
        if let Ok(Some(d)) = r {
            lat.push(d);
        }
    }
    lat.sort();
    OverloadStats {
        accepted: lat.len(),
        rejected: rejected.load(Ordering::Relaxed) as usize,
        p50: percentile(&lat, 0.5),
        p99: percentile(&lat, 0.99),
        peak_in_flight: peak.load(Ordering::SeqCst),
    }
}

// ============ 3. キュー: 無制限 vs 有界（背圧） ============

#[derive(Debug)]
pub struct QueueStats {
    pub max_queue_len: usize,
    pub p99_wait: Duration,
    pub producer_finished_at: Duration,
}

/// 生産 1000 件/秒、消費 500 件/秒 を 2 秒間。
/// `bound=None` は無制限キュー（`unbounded_channel` と同じ振る舞いをさせるため、事実上無限の容量にする）。
pub async fn run_queue(bound: Option<usize>) -> QueueStats {
    let start = Instant::now();
    let n = 2000;
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Instant>(bound.unwrap_or(1 << 24));
    let qlen = Arc::new(AtomicUsize::new(0));
    let q = qlen.clone();
    let consumer = tokio::spawn(async move {
        let mut waits = Vec::new();
        while let Some(sent) = rx.recv().await {
            q.fetch_sub(1, Ordering::SeqCst);
            waits.push(sent.elapsed());
            sleep(Duration::from_millis(2)).await;
        }
        waits
    });
    let mut max_q = 0;
    for _ in 0..n {
        // 有界なら空きが出るまでここで待つ（背圧が生産者に伝わる）
        let Ok(permit) = tx.reserve().await else { break };
        permit.send(Instant::now());
        max_q = max_q.max(qlen.fetch_add(1, Ordering::SeqCst) + 1);
        sleep(Duration::from_millis(1)).await;
    }
    let producer_finished_at = start.elapsed();
    drop(tx);
    let mut w = consumer.await.unwrap_or_default();
    w.sort();
    QueueStats { max_queue_len: max_q, p99_wait: percentile(&w, 0.99), producer_finished_at }
}

// ============ 4. パニックの隔離 ============

#[derive(Debug)]
pub struct PanicStats {
    pub tasks: usize,
    pub panicked: usize,
    pub completed: usize,
    pub runtime_alive: bool,
}

/// 100 タスク中 1 割がパニックしても、他のタスクとランタイムは生きているか
pub async fn run_panic_isolation() -> PanicStats {
    let mut set = tokio::task::JoinSet::new();
    for i in 0..100u32 {
        set.spawn(async move {
            sleep(Duration::from_millis(u64::from(i % 7))).await;
            assert!(i % 10 != 3, "simulated bug in task {i}");
            i
        });
    }
    let (mut ok, mut panicked) = (0, 0);
    while let Some(r) = set.join_next().await {
        match r {
            Ok(_) => ok += 1,
            Err(e) if e.is_panic() => panicked += 1,
            Err(_) => {}
        }
    }
    let alive = tokio::spawn(async { 42 }).await.ok() == Some(42);
    PanicStats { tasks: 100, panicked, completed: ok, runtime_alive: alive }
}

/// std::sync::Mutex はパニックで「毒」状態になる。`into_inner` で回復できるか確かめる
#[must_use]
pub fn mutex_poison_recovery() -> (bool, u32) {
    let m = Arc::new(std::sync::Mutex::new(0u32));
    let m2 = m.clone();
    let _ = std::thread::spawn(move || {
        let mut g = m2.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *g += 1;
        panic!("poison it");
    })
    .join();
    let poisoned = m.is_poisoned();
    let v = *m.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    (poisoned, v)
}

// ============ 5. グレースフルシャットダウン ============

#[derive(Debug)]
pub struct ShutdownStats {
    pub started: usize,
    pub finished: usize,
    pub aborted: usize,
    pub shutdown_took: Duration,
}

/// 処理時間 50〜250ms のリクエストが流れている途中で停止信号を受ける。
/// `drain=None`: 即 abort。`drain=Some(d)`: 新規受付を止め、最大 d だけ完了を待ってから残りを abort。
pub async fn run_shutdown(drain: Option<Duration>) -> ShutdownStats {
    let (stop_tx, mut stop_rx) = tokio::sync::watch::channel(false);
    let finished = Arc::new(AtomicUsize::new(0));
    let mut set = tokio::task::JoinSet::new();
    let mut rng = Rng::new(7);
    let mut started = 0;
    tokio::spawn(async move {
        sleep(Duration::from_millis(500)).await;
        let _ = stop_tx.send(true);
    });
    loop {
        tokio::select! {
            _ = stop_rx.changed() => break,
            () = sleep(Duration::from_millis(5)) => {
                let work = Duration::from_millis(50 + rng.next_u64() % 200);
                let f = finished.clone();
                set.spawn(async move { sleep(work).await; f.fetch_add(1, Ordering::SeqCst); });
                started += 1;
            }
        }
    }
    let t = Instant::now();
    if let Some(d) = drain {
        let _ = timeout(d, async { while set.join_next().await.is_some() {} }).await;
    }
    set.shutdown().await;
    let done = finished.load(Ordering::SeqCst);
    ShutdownStats {
        started,
        finished: done,
        aborted: started - done,
        shutdown_took: t.elapsed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn retry_beats_naive_on_tail_latency() {
        let naive = run_retry_experiment(RetryPolicy::Naive, 500, 1).await;
        let best = run_retry_experiment(
            RetryPolicy::TimeoutRetryDeadline {
                per_try: Duration::from_millis(100),
                max_attempts: 3,
                deadline: Duration::from_millis(500),
            },
            500,
            1,
        )
        .await;
        assert!(best.p99 < naive.p99);
        assert!(best.ok > naive.ok);
    }

    #[tokio::test(start_paused = true)]
    async fn shedding_bounds_latency() {
        let d = Duration::from_secs(2);
        let unb = run_overload(Admission::Unbounded, 1000, d, 8).await;
        let shed = run_overload(Admission::SemaphoreShed(8), 1000, d, 8).await;
        assert!(shed.p99 < unb.p99);
        assert!(shed.rejected > 0);
    }

    #[tokio::test(start_paused = true)]
    async fn bounded_queue_caps_memory() {
        let unb = run_queue(None).await;
        let b = run_queue(Some(64)).await;
        assert!(b.max_queue_len <= 65);
        assert!(unb.max_queue_len > 500);
    }

    #[tokio::test(start_paused = true)]
    async fn panics_are_isolated() {
        let s = run_panic_isolation().await;
        assert_eq!(s.panicked, 10);
        assert_eq!(s.completed, 90);
        assert!(s.runtime_alive);
    }

    #[test]
    fn poison_is_recoverable() {
        assert_eq!(mutex_poison_recovery(), (true, 1));
    }

    #[tokio::test(start_paused = true)]
    async fn drain_finishes_inflight() {
        let hard = run_shutdown(None).await;
        let soft = run_shutdown(Some(Duration::from_secs(1))).await;
        assert!(hard.aborted > 0);
        assert_eq!(soft.aborted, 0);
    }
}
