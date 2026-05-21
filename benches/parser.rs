use criterion::{Criterion, black_box, criterion_group, criterion_main};
use ferrish::parser::Parser;
use ferrish::scanner::Scanner;

const TYPICAL_LINE: &[u8] = b"echo hello world | cat -n > out.txt";
const PIPELINE_5: &[u8] = b"echo foo | tr a-z A-Z | cat | grep F | head -1";

fn bench_scan_typical(c: &mut Criterion) {
    c.bench_function("scan_typical_line", |b| {
        b.iter(|| {
            let mut sc = Scanner::new();
            sc.push(black_box(TYPICAL_LINE));
            black_box(sc.finalize().count())
        });
    });
}

fn bench_parse_typical(c: &mut Criterion) {
    c.bench_function("parse_typical_line", |b| {
        b.iter(|| {
            let mut sc = Scanner::new();
            sc.push(black_box(TYPICAL_LINE));
            black_box(Parser::new(sc.finalize()).count())
        });
    });
}

fn bench_parse_pipeline_5(c: &mut Criterion) {
    c.bench_function("parse_pipeline_5_stages", |b| {
        b.iter(|| {
            let mut sc = Scanner::new();
            sc.push(black_box(PIPELINE_5));
            black_box(Parser::new(sc.finalize()).count())
        });
    });
}

criterion_group!(
    benches,
    bench_scan_typical,
    bench_parse_typical,
    bench_parse_pipeline_5
);
criterion_main!(benches);
