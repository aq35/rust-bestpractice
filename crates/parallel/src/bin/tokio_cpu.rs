//! async ランタイム上で CPU 処理をどこで実行するかで、他タスクの遅延がどう変わるかを測る。
use parallel::{CpuMode, tick_latency_under_cpu_load};

fn main() {
    let workers = 2;
    println!("| mode | worker_threads | jobs | 1ms タイマーの最大遅延 |");
    println!("|---|---|---|---|");
    for mode in [CpuMode::Inline, CpuMode::SpawnBlocking, CpuMode::Rayon] {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(workers)
            .enable_all()
            .build()
            .expect("runtime");
        let worst = rt.block_on(tick_latency_under_cpu_load(mode, 8));
        println!("| {mode:?} | {workers} | 8 | {:.1} ms |", worst.as_secs_f64() * 1e3);
    }
}
