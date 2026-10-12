//! 単一スレッド性能の実験対象。各モジュールは「素朴な版」と「推奨する版」を並べ、
//! `benches/perf.rs` で同じ入力に対して測る。テストで両者が同じ結果を返すことを保証する。

pub mod alloc {
    /// 容量を指定せずに push（再確保が log2(n) 回起きる）
    #[must_use]
    pub fn push_no_capacity(n: usize) -> Vec<u64> {
        let mut v = Vec::new();
        for i in 0..n {
            v.push(i as u64 * 3);
        }
        v
    }

    /// 容量を先に確保
    #[must_use]
    pub fn push_with_capacity(n: usize) -> Vec<u64> {
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            v.push(i as u64 * 3);
        }
        v
    }

    /// イテレータ + collect（`size_hint` から容量が決まる）
    #[must_use]
    pub fn collect(n: usize) -> Vec<u64> {
        (0..n).map(|i| i as u64 * 3).collect()
    }

    /// ループ内で毎回バッファを作る
    #[must_use]
    pub fn fresh_buffer_each_iter(lines: &[&str]) -> usize {
        let mut total = 0;
        for l in lines {
            let buf: String = l.chars().rev().collect();
            total += buf.len();
        }
        total
    }

    /// バッファを使い回す（clear は容量を保持する）
    #[must_use]
    pub fn reuse_buffer(lines: &[&str]) -> usize {
        let mut total = 0;
        let mut buf = String::new();
        for l in lines {
            buf.clear();
            buf.extend(l.chars().rev());
            total += buf.len();
        }
        total
    }
}

pub mod strings {
    use std::fmt::Write as _;

    #[must_use]
    pub fn format_concat(items: &[u32]) -> String {
        let mut s = String::new();
        for i in items {
            s = format!("{s}{i},");
        }
        s
    }

    #[must_use]
    pub fn write_into(items: &[u32]) -> String {
        let mut s = String::with_capacity(items.len() * 6);
        for i in items {
            let _ = write!(s, "{i},");
        }
        s
    }

    #[must_use]
    pub fn push_str_tostring(items: &[u32]) -> String {
        let mut s = String::new();
        for i in items {
            s.push_str(&i.to_string());
            s.push(',');
        }
        s
    }
}

pub mod hashing {
    use std::collections::HashMap;

    pub type StdMap<K, V> = HashMap<K, V>;
    pub type FxMap<K, V> = rustc_hash::FxHashMap<K, V>;
    pub type AMap<K, V> = HashMap<K, V, ahash::RandomState>;

    /// 任意の Map 型で insert → lookup を行う共通ルーチン
    pub fn insert_lookup<M>(keys: &[u64], mut m: M) -> u64
    where
        M: MapLike<u64, u64>,
    {
        for &k in keys {
            m.put(k, k ^ 0xdead);
        }
        let mut acc = 0u64;
        for &k in keys {
            acc = acc.wrapping_add(m.fetch(&k).unwrap_or(0));
        }
        acc
    }

    pub fn insert_lookup_str<M>(keys: &[String], mut m: M) -> usize
    where
        M: MapLike<String, usize>,
    {
        for (i, k) in keys.iter().enumerate() {
            m.put(k.clone(), i);
        }
        keys.iter().filter_map(|k| m.fetch(k)).sum()
    }

    pub trait MapLike<K, V> {
        fn put(&mut self, k: K, v: V);
        fn fetch(&self, k: &K) -> Option<V>;
    }

    impl<K, V, S> MapLike<K, V> for HashMap<K, V, S>
    where
        K: std::hash::Hash + Eq,
        V: Copy,
        S: std::hash::BuildHasher,
    {
        fn put(&mut self, k: K, v: V) {
            self.insert(k, v);
        }
        fn fetch(&self, k: &K) -> Option<V> {
            self.get(k).copied()
        }
    }
}

pub mod dispatch {
    pub trait Shape {
        fn area(&self) -> f64;
    }
    pub struct Circle(pub f64);
    pub struct Rect(pub f64, pub f64);
    impl Shape for Circle {
        fn area(&self) -> f64 {
            std::f64::consts::PI * self.0 * self.0
        }
    }
    impl Shape for Rect {
        fn area(&self) -> f64 {
            self.0 * self.1
        }
    }

    pub enum ShapeE {
        Circle(f64),
        Rect(f64, f64),
    }
    impl ShapeE {
        #[must_use]
        pub fn area(&self) -> f64 {
            match self {
                ShapeE::Circle(r) => std::f64::consts::PI * r * r,
                ShapeE::Rect(w, h) => w * h,
            }
        }
    }

    #[must_use]
    pub fn sum_dyn(v: &[Box<dyn Shape>]) -> f64 {
        v.iter().map(|s| s.area()).sum()
    }
    #[must_use]
    pub fn sum_enum(v: &[ShapeE]) -> f64 {
        v.iter().map(ShapeE::area).sum()
    }
    /// 型が 1 種類に決まる場合はジェネリクス（単相化）
    #[must_use]
    pub fn sum_generic<S: Shape>(v: &[S]) -> f64 {
        v.iter().map(Shape::area).sum()
    }
}

pub mod iter {
    /// 添字アクセス（境界チェックが残りやすい）
    #[must_use]
    #[allow(clippy::needless_range_loop)] // 比較のためにわざと添字で書く
    pub fn dot_index(a: &[f32], b: &[f32]) -> f32 {
        let mut s = 0.0;
        for i in 0..a.len() {
            s += a[i] * b[i];
        }
        s
    }

    /// zip（境界チェックが消える）。f32 の加算順は保たれるのでベクトル化はされない
    #[must_use]
    pub fn dot_zip(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }

    /// 8 本の独立アキュムレータ: 加算順を変えてよいと明示することで SIMD 化を許す
    #[must_use]
    pub fn dot_chunks8(a: &[f32], b: &[f32]) -> f32 {
        let mut acc = [0.0f32; 8];
        let ca = a.chunks_exact(8);
        let cb = b.chunks_exact(8);
        let (ra, rb) = (ca.remainder(), cb.remainder());
        for (x, y) in ca.zip(cb) {
            for i in 0..8 {
                acc[i] += x[i] * y[i];
            }
        }
        let tail: f32 = ra.iter().zip(rb).map(|(x, y)| x * y).sum();
        acc.iter().sum::<f32>() + tail
    }

    #[must_use]
    #[allow(clippy::needless_range_loop)]
    pub fn sum_u64_index(v: &[u64]) -> u64 {
        let mut s = 0u64;
        for i in 0..v.len() {
            s = s.wrapping_add(v[i]);
        }
        s
    }
    #[must_use]
    pub fn sum_u64_iter(v: &[u64]) -> u64 {
        v.iter().fold(0u64, |a, &b| a.wrapping_add(b))
    }
}

pub mod layout {
    /// Array of Structs: 1 フィールドだけ読むときも 64 バイト全体がキャッシュに載る
    #[derive(Clone, Copy, Default)]
    pub struct Particle {
        pub pos: [f32; 3],
        pub vel: [f32; 3],
        pub mass: f32,
        pub charge: f32,
        pub pad: [f32; 8],
    }

    #[must_use]
    pub fn total_mass_aos(p: &[Particle]) -> f32 {
        p.iter().map(|p| p.mass).sum()
    }

    /// Struct of Arrays
    #[derive(Default)]
    pub struct Particles {
        pub pos: Vec<[f32; 3]>,
        pub vel: Vec<[f32; 3]>,
        pub mass: Vec<f32>,
        pub charge: Vec<f32>,
    }

    #[must_use]
    pub fn total_mass_soa(p: &Particles) -> f32 {
        p.mass.iter().sum()
    }
}

pub mod errors {
    #[derive(Debug, thiserror::Error)]
    pub enum ParseErr {
        #[error("empty input")]
        Empty,
        #[error("bad digit {0:?}")]
        BadDigit(char),
    }

    /// 型付きエラー（enum）: 失敗時もヒープ確保なし
    pub fn parse_typed(s: &str) -> Result<u64, ParseErr> {
        if s.is_empty() {
            return Err(ParseErr::Empty);
        }
        let mut n = 0u64;
        for c in s.chars() {
            let d = c.to_digit(10).ok_or(ParseErr::BadDigit(c))?;
            n = n * 10 + u64::from(d);
        }
        Ok(n)
    }

    /// anyhow: 失敗時に Box 確保 + (環境変数次第で) バックトレース取得
    pub fn parse_anyhow(s: &str) -> anyhow::Result<u64> {
        if s.is_empty() {
            anyhow::bail!("empty input");
        }
        let mut n = 0u64;
        for c in s.chars() {
            let d = c.to_digit(10).ok_or_else(|| anyhow::anyhow!("bad digit {c:?}"))?;
            n = n * 10 + u64::from(d);
        }
        Ok(n)
    }
}

pub mod small {
    use smallvec::SmallVec;

    #[must_use]
    pub fn tokens_vec(lines: &[&str]) -> usize {
        lines.iter().map(|l| l.split(' ').collect::<Vec<_>>().len()).sum()
    }

    #[must_use]
    pub fn tokens_smallvec(lines: &[&str]) -> usize {
        lines
            .iter()
            .map(|l| l.split(' ').collect::<SmallVec<[&str; 8]>>().len())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_variants_agree() {
        assert_eq!(alloc::push_no_capacity(1000), alloc::push_with_capacity(1000));
        assert_eq!(alloc::collect(1000), alloc::push_with_capacity(1000));
        let lines = ["abc", "de", "ふが"];
        assert_eq!(alloc::fresh_buffer_each_iter(&lines), alloc::reuse_buffer(&lines));
    }

    #[test]
    fn string_variants_agree() {
        let v: Vec<u32> = (0..100).collect();
        let a = strings::format_concat(&v);
        assert_eq!(a, strings::write_into(&v));
        assert_eq!(a, strings::push_str_tostring(&v));
    }

    #[test]
    fn hash_variants_agree() {
        use hashing::*;
        let keys: Vec<u64> = (0..1000).map(|i| i * 7919).collect();
        let a = insert_lookup(&keys, StdMap::default());
        assert_eq!(a, insert_lookup(&keys, FxMap::default()));
        assert_eq!(a, insert_lookup(&keys, AMap::default()));
    }

    #[test]
    fn dot_variants_close() {
        let a: Vec<f32> = (0..1001).map(|i| (i % 7) as f32).collect();
        let b: Vec<f32> = (0..1001).map(|i| (i % 5) as f32).collect();
        let r = iter::dot_index(&a, &b);
        assert!((r - iter::dot_zip(&a, &b)).abs() < 1e-3);
        assert!((r - iter::dot_chunks8(&a, &b)).abs() < 1e-3);
    }

    #[test]
    fn errors_agree() {
        assert_eq!(errors::parse_typed("123").ok(), errors::parse_anyhow("123").ok());
        assert!(errors::parse_typed("1x").is_err());
        assert!(errors::parse_anyhow("").is_err());
    }

    #[test]
    fn smallvec_agree() {
        let l = ["a b c", "d e f g h i j k l"];
        assert_eq!(small::tokens_vec(&l), small::tokens_smallvec(&l));
    }
}
