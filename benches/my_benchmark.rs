use criterion::{criterion_group, criterion_main, Criterion};
use std::{hint::black_box, ops::Neg};

#[expect(dead_code)]
enum Integer {
    Incr,
    Decr,
    IncrBy(i64),
    DecrBy(i64),
}

enum IntegerOperation {
    Add(i64),
    Subtract(i64),
}

impl IntegerOperation {
    fn apply(&self, value: i64) -> Option<i64> {
        match self {
            IntegerOperation::Add(n) => value.checked_add(*n),
            IntegerOperation::Subtract(n) => value.checked_sub(*n),
        }
    }
}

impl Integer {
    pub fn execute_enum(&self) -> Option<i64> {
        let (_, operation) = self.operation_enum();
        operation.apply(10)
    }

    pub fn execute(&self) -> Option<i64> {
        let (_, operation) = self.operation();
        operation(10)
    }

    fn operation_enum(&self) -> (i64, IntegerOperation) {
        match self {
            Integer::Incr => (1, IntegerOperation::Add(1)),
            Integer::Decr => (-1, IntegerOperation::Subtract(1)),
            Integer::IncrBy(v) => (*v, IntegerOperation::Add(*v)),
            Integer::DecrBy(v) => (v.neg(), IntegerOperation::Subtract(*v)),
        }
    }

    fn operation(&self) -> (i64, Box<dyn Fn(i64) -> Option<i64> + '_>) {
        match self {
            Integer::Incr => (1, Box::new(|i: i64| i.checked_add(1))),
            Integer::Decr => (-1, Box::new(|i: i64| i.checked_sub(1))),
            Integer::IncrBy(v) => (*v, Box::new(|i: i64| i.checked_add(*v))),
            Integer::DecrBy(v) => (v.neg(), Box::new(|i: i64| i.checked_sub(*v))),
        }
    }
}

fn my_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("Integer");
    group.bench_function("execute", |b| b.iter(|| black_box(Integer::Incr.execute())));
    group.bench_function("execute_enum", |b| b.iter(|| black_box(Integer::Incr.execute_enum())));
    group.finish();
}

criterion_group!(benches, my_benchmark);
criterion_main!(benches);
