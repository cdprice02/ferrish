use criterion::{Criterion, black_box, criterion_group, criterion_main};
use ferrish::lexer::Lexer;
use ferrish::parser::Parser;

const TYPICAL_LINE: &[u8] = b"echo hello world | cat -n > out.txt";
const PIPELINE_5: &[u8] = b"echo foo | tr a-z A-Z | cat | grep F | head -1";

fn bench_lex_typical(c: &mut Criterion) {
    c.bench_function("lex_typical_line", |b| {
        b.iter(|| {
            let mut lex = Lexer::new();
            lex.push(black_box(TYPICAL_LINE));
            lex.push(b"\n");
            lex.finalize();
            black_box(lex.count())
        });
    });
}

fn bench_parse_typical(c: &mut Criterion) {
    c.bench_function("parse_typical_line", |b| {
        b.iter(|| {
            let mut lex = Lexer::new();
            lex.push(black_box(TYPICAL_LINE));
            lex.push(b"\n");
            lex.finalize();
            black_box(Parser::new(lex).count())
        });
    });
}

fn bench_parse_pipeline_5(c: &mut Criterion) {
    c.bench_function("parse_pipeline_5_stages", |b| {
        b.iter(|| {
            let mut lex = Lexer::new();
            lex.push(black_box(PIPELINE_5));
            lex.push(b"\n");
            lex.finalize();
            black_box(Parser::new(lex).count())
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
