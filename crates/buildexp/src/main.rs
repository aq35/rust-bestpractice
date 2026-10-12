//! ビルド設定（プロファイル・リンカ）の違いで「ビルド時間 / バイナリサイズ / 実行時間」がどう変わるかを測る対象。
//! JSON の生成・解析と、並列の集計をする。
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Order {
    id: u64,
    user: String,
    items: Vec<Item>,
}

#[derive(Serialize, Deserialize)]
struct Item {
    sku: String,
    qty: u32,
    price_cents: u64,
}

fn main() -> Result<(), serde_json::Error> {
    let n: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(200_000);
    let orders: Vec<Order> = (0..n)
        .map(|i| Order {
            id: i,
            user: format!("user{}", i % 1000),
            items: (0..(i % 5 + 1))
                .map(|j| Item { sku: format!("SKU-{j}"), qty: (j + 1) as u32, price_cents: 100 + i % 997 })
                .collect(),
        })
        .collect();
    let json = serde_json::to_string(&orders)?;
    let back: Vec<Order> = serde_json::from_str(&json)?;
    let total: u64 = back
        .par_iter()
        .flat_map(|o| &o.items)
        .map(|it| u64::from(it.qty) * it.price_cents)
        .sum();
    let rt = tokio::runtime::Builder::new_current_thread().build().expect("rt");
    let len = rt.block_on(async { json.len() });
    println!("orders={n} json_bytes={len} total_cents={total}");
    Ok(())
}
