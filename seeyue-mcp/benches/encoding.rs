// benches/encoding.rs
//
// Criterion benchmarks for encoding detection and safe read/write.
// Run: cargo bench --bench encoding

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use seeyue_mcp::encoding::{detect_encoding, sha256_hex};

// ─── Fixtures ────────────────────────────────────────────────────────────────

const ASCII_1K: &[u8] = b"fn main() {\n    println!(\"Hello, world!\");\n}\n"
    .as_slice();

const UTF8_CHINESE: &[u8] =
    "// 这是一段中文注释\nfn main() {\n    let s = \"你好世界\";\n}\n".as_bytes();

const UTF8_BOM: &[u8] = b"\xEF\xBB\xBFfn main() {}\n";
const UTF16_LE_BOM: &[u8] = b"\xFF\xFEf\x00n\x00 \x00m\x00a\x00i\x00n\x00";

// 4KB sample (typical small source file)
fn make_4k_ascii() -> Vec<u8> {
    b"// source file\nfn foo() -> u32 { 42 }\n"
        .iter()
        .cloned()
        .cycle()
        .take(4096)
        .collect()
}

// ─── detect_encoding ─────────────────────────────────────────────────────────

fn bench_detect_encoding(c: &mut Criterion) {
    let data_4k = make_4k_ascii();

    let mut group = c.benchmark_group("detect_encoding");

    group.bench_function("ascii_small", |b| {
        b.iter(|| detect_encoding(ASCII_1K))
    });
    group.bench_function("ascii_4k", |b| {
        b.iter(|| detect_encoding(&data_4k))
    });
    group.bench_function("utf8_chinese", |b| {
        b.iter(|| detect_encoding(UTF8_CHINESE))
    });
    group.bench_function("utf8_bom", |b| {
        b.iter(|| detect_encoding(UTF8_BOM))
    });
    group.bench_function("utf16_le_bom", |b| {
        b.iter(|| detect_encoding(UTF16_LE_BOM))
    });

    group.finish();
}

// ─── sha256_hex ───────────────────────────────────────────────────────────────

fn bench_sha256(c: &mut Criterion) {
    let data_4k = make_4k_ascii();
    let data_64k: Vec<u8> = data_4k.iter().cloned().cycle().take(65536).collect();

    let mut group = c.benchmark_group("sha256_hex");
    for (label, data) in &[
        ("1k",  ASCII_1K.to_vec()),
        ("4k",  data_4k.clone()),
        ("64k", data_64k.clone()),
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            data,
            |b, d| b.iter(|| sha256_hex(d)),
        );
    }
    group.finish();
}

// ─── Entry point ──────────────────────────────────────────────────────────────

criterion_group!(benches, bench_detect_encoding, bench_sha256);
criterion_main!(benches);
