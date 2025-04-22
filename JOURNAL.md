## 2025-03-28
<details>
Redis is not able to handle a benchmark with 50 clients and 10k requests. Many threads return: `failed to read from socket: Connection reset by peer`. Trying to use `RwLock` instead of `Mutex`: read performance increases (with less than 10k requests) but the issue persists. Realized that `tokio::sync::Mutex` or `tokio::sync::RwLock` are not required as the struct is not held over an `await` point. Using `std::sync::Mutex` doesn't solve the issue.
</details>

## 2025-03-29
<details>
Ok, debug time. Just return a simple string instead of deserializing and processing the message. Surprise surprise, the benchmark holds.
Now only deserialize... BAM! Benchmark can't complete.

Read carefully the docs about `read_buf`: if the function returns `0`, either EOF has been reached (in our case, the client has closed the connection) or the buffer has a remaining capacity of `0`. Adding more logs over the buffer's len and capacity and also assigning a uuid to each thread to track the failures. Noticing that sometimes the message is incomplete, multiple times on the same thread. Noticing that the buffer is resized sometimes. Surprise surprise: the buffer must be cleared 🤦 Buffer size reduced from 4KB to 1KB.

Performance is good, even better than the original Redis, but while the original Redis consumes 80% of the CPU, our Redis consumes 280% of the CPU, at least according to `top`.

TODO:
- evaluate `std::sync::Mutex` over `tokio::sync::Mutex` for the performance side of things
- investigate how to reduce the CPU usage
</details>

## 2025-04-01
<details>
Added the missing tests for the `GET` command. Fixed the `PING` and `ECHO` tests.

Advancing to [step #5](https://codingchallenges.fyi/challenges/challenge-redis/#step-5). After reading the Redis docs and digging online, I currently see three ways to implement the expiration policy:
- store the timestamp as part of the value (#1)
- store the timestamp and the key as a tuple in a separate `BTreeSet` (#2)
- store the timestamp and the key as a tuple in a separate `BTreeSet` and the key -> timestamp association in a separate `HashMap` (#3)

Active expiration can be implemented as a cron job in a separate tokio task, kinda like a garbage-collector.

### Pros and cons #1
- 👍 checking for expiration upon `GET` requests is trivial
- 👍 `SET` operations are trivial
- 👎 active expiration can be quite CPU intensive when there are a lot of elements; this can be mitigated with the random sampling strategy that Redis used in earlier implementations, where only a subset of keys are tested for expiration and the size of the sample is adjusted, depending on how many expired keys have been found over that sample

### Pros and cons #2
- 👍 active expiration is space-efficient as it gets rid of all expired keys; in a linear use-case scenario, the more frequently the task runs the less keys it has to remove, making it less CPU intensive
- 👎 must search for the key expiration time on `GET` requests
- 👎 must search and update the key expiration time on `SET` requests

### Pros and cons #3
- 👍 same as #2
- 👍 retrieval of the key expiration time is fast on `GET` requests, the timestamp stored in the `HashMap` is now used to remove the entry in the `BTreeSet`
- 👍 no need to search for the key expiration time on `SET` requests, just update the entry in the three data structures
- 👎 one additional operation is performed everytime
- 👎 the expiration keys now take twice as much space compared with #2

Given that I want to prioritize UX while accepting a good compromise over memory/storage used, I will go with either #1 or #3.

The main pain-point of #3 is the space used. Let us assume that in the worst-case scenario, 10M expiration keys are stored at any given time, with each key being ~60 ASCII chars on average and the timestamp stored as `SystemTime`, which takes 16 bytes.
Each `String` takes: 60 bytes + 24 bytes for pointer, length and capacity. Total space taken per key: 100 bytes.

10M keys * 100 bytes = 1GB of memory/storage used. This might be acceptable in certain scenarios.
</details>

## 2025-04-02
<details>
For now I decided to go with #1: store expiration as part of the value. Passive expiration implemented. Some bugs fixed and tests added
</details>

## 2025-04-03
<details>

Realized that iteration over HashMap is not random on the same program execution by just using `.iter().take(n)`. Either use a separate data structure or change strategy.

A `Vec` would work as a separate data structure but it would be unfeasible for removals on `GET` requests (i.e. passive expiration).

Found that the crates `indexmap` and `rand` might give me what I need. Algorithm implemented, not tested yet.

Problem: the same key is retrieved multiple times. Look into `choose_multiple` and `sample` of `rand`
</details>

## 2025-04-08
<details>

After reading the documentation of `choose_multiple` and `rand`, I decided to stick with `sample` for now, as the sample size is small.
In case the sample size is increased, it might be worth differentiate the algorithm and use one function or another, as when the length of the map is big and sample size is close to the length of the map, `choose_multiple`'s performance is better.

Active expiration tests added. `EXISTS` added.

TODO:
- check memory and CPU usage of the active expiration
- think about how to organize parser and cmd
</details>

## 2025-04-09
<details>

`SET` tests, `EXISTS` tests and `DEL` command. Some refactoring: command constant types, client error moved to error file.
</details>

## 2025-04-10
<details>

Made types mod private. Digged a bit into the other commands to understand how to properly store integers and lists. Most likely the value will be changed from `String` to `enum`, with variants being: `String`, `i64` and (probably) `VecDeque`.
</details>

## 2025-04-14
<details>

Expiration logic moved to `Db`. Change `Object` value to `enum`.
</details>

## 2025-04-15
<details>

First version of `Incr` implemented and expiration status moved to the object itself.
</details>

## 2025-04-16
<details>

`Incr` tests. `Decr` implemented. Arithmetic logic unified and moved to new execution module. Code prepared to handle possible implementations of `INCRBY` and `DECRBY`.

Had to use the `Box` smart pointer to return two different closures from the `operation` method.
</details>

## 2025-04-17
<details>

`INCR` and `DECR` tests moved. `INCRBY` and `DECRBY` implemented. Had to implement the parser for incrby and decrby to convert String to i64. Realized that the set parser takes a `Vec<String>` during the process; as the copy is not necessary, it has been changed to a `&[String]`.
</details>

## 2025-04-22
<details>

I watched [this video](https://youtu.be/Sv6Bswfjnzo?feature=shared&t=157) to refresh my knowledge around static and dynamic dispatch. Generics instead of trait objects can't be used in the `operation` function to achieve static dispatch, as no two closures, even if identical, are of the same type. Enums could provide a faster execution with the following code:
```rust
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
    fn operation_enum(&self) -> (i64, IntegerOperation) {
        match self {
            Integer::Incr => (1, IntegerOperation::Add(1)),
            Integer::Decr => (-1, IntegerOperation::Subtract(1)),
            Integer::IncrBy(v) => (*v, IntegerOperation::Add(*v)),
            Integer::DecrBy(v) => (v.neg(), IntegerOperation::Subtract(*v)),
        }
    }
}
```

However, after running a [benchmark](./benches/my_benchmark.rs) with [criterion](https://crates.io/crates/criterion), I've noticed there's no real benefit in using enums. I will leave the trait object as it's way easier to read.

TODO:
- evaluate `Cow` with &str as suggested in the Rustikon video
</details>
