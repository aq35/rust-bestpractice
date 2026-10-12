#![allow(clippy::wildcard_imports)] // 実験対象を一括で取り込む
//! 可用性の実験をすべて実行し、Markdown の表で出力する（時間は仮想時間）。
use availability::*;
use std::time::Duration;

fn ms(d: Duration) -> String {
    format!("{:.0} ms", d.as_secs_f64() * 1e3)
}

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .expect("runtime");
    rt.block_on(async {
        println!("### 1. タイムアウトとリトライ（上流: 70% 成功 / 20% エラー / 10% 30 秒ハング、1000 回）\n");
        println!("| 方針 | 成功率 | p50 | p99 | 最大 |");
        println!("|---|---|---|---|---|");
        let per_try = Duration::from_millis(100);
        for (name, p) in [
            ("なし", RetryPolicy::Naive),
            ("タイムアウト 100ms", RetryPolicy::TimeoutOnly(per_try)),
            ("タイムアウト + リトライ 3 回", RetryPolicy::TimeoutRetry { per_try, max_attempts: 3 }),
            ("タイムアウト + リトライ 5 回", RetryPolicy::TimeoutRetry { per_try, max_attempts: 5 }),
            ("+ 全体期限 300ms（5 回まで）", RetryPolicy::TimeoutRetryDeadline { per_try, max_attempts: 5, deadline: Duration::from_millis(300) }),
        ] {
            let s = run_retry_experiment(p, 1000, 42).await;
            println!("| {name} | {:.1}% | {} | {} | {} |", s.ok as f64 * 100.0 / s.total as f64, ms(s.p50), ms(s.p99), ms(s.max));
        }

        println!("\n### 2. 過負荷（容量 8 同時 × 10ms = 800 rps のサーバーに 1000 rps を 2 秒）\n");
        println!("| 受け入れ方 | 受理 | 拒否 | p50 | p99 | 同時実行の最大 |");
        println!("|---|---|---|---|---|---|");
        for (name, a) in [
            ("無制限", Admission::Unbounded),
            ("Semaphore(8) で待たせる", Admission::SemaphoreQueue(8)),
            ("Semaphore(8) で即拒否", Admission::SemaphoreShed(8)),
        ] {
            let s = run_overload(a, 1000, Duration::from_secs(2), 8).await;
            println!("| {name} | {} | {} | {} | {} | {} |", s.accepted, s.rejected, ms(s.p50), ms(s.p99), s.peak_in_flight);
        }

        println!("\n### 3. キュー（生産 1000/秒、消費 500/秒、2000 件）\n");
        println!("| キュー | 最大の長さ | 待ち時間 p99 | 生産者が終わった時刻 |");
        println!("|---|---|---|---|");
        for (name, b) in [("無制限", None), ("有界 64", Some(64))] {
            let s = run_queue(b).await;
            println!("| {name} | {} | {} | {} |", s.max_queue_len, ms(s.p99_wait), ms(s.producer_finished_at));
        }

        println!("\n### 4. パニックの隔離\n");
        let p = run_panic_isolation().await;
        println!("- tokio: {} タスク中 {} がパニック → 完了 {} / ランタイム生存 {}", p.tasks, p.panicked, p.completed, p.runtime_alive);
        let (poisoned, v) = mutex_poison_recovery();
        println!("- std::sync::Mutex: パニック後 poisoned={poisoned}、`PoisonError::into_inner` で値 {v} を回収できた");

        println!("\n### 5. グレースフルシャットダウン（500ms 時点で停止信号、処理時間 50〜250ms）\n");
        println!("| 方法 | 受付 | 完了 | 中断 | 停止にかかった時間 |");
        println!("|---|---|---|---|---|");
        for (name, d) in [("即 abort", None), ("受付停止 + 最大 1 秒待つ", Some(Duration::from_secs(1)))] {
            let s = run_shutdown(d).await;
            println!("| {name} | {} | {} | {} | {} |", s.started, s.finished, s.aborted, ms(s.shutdown_took));
        }
    });
}
