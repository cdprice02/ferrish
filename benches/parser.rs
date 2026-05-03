use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ferrish::input::Input;
use ferrish::{lexer, parser};

const TYPICAL_LINE: &[u8] = b"echo hello world | cat -n > out.txt";
const PIPELINE_5: &[u8] = b"echo foo | tr a-z A-Z | cat | grep F | head -1";

fn bench_lex_typical(c: &mut Criterion) {
    c.bench_function("lex_typical_line", |b| {
        b.iter(|| {
            let input = Input::new(black_box(TYPICAL_LINE));
            lexer::lex(&input).for_each(drop);
        });
    });
}

fn bench_parse_typical(c: &mut Criterion) {
    c.bench_function("parse_typical_line", |b| {
        b.iter(|| {
            let input = Input::new(black_box(TYPICAL_LINE));
            parser::parse(&input).for_each(drop);
        });
    });
}

fn bench_parse_pipeline_5(c: &mut Criterion) {
    c.bench_function("parse_pipeline_5_stages", |b| {
        b.iter(|| {
            let input = Input::new(black_box(PIPELINE_5));
            parser::parse(&input).for_each(drop);
        });
    });
}

criterion_group!(
    benches,
    bench_lex_typical,
    bench_parse_typical,
    bench_parse_pipeline_5
);
criterion_main!(benches);
