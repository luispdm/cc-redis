## 2024-03-28
Redis is not able to handle a benchmark with 50 clients and 10k requests. Many threads return: `failed to read from socket: Connection reset by peer`. Trying to use `RwLock` instead of `Mutex`: read performance increases (with less than 10k requests) but the issue persists. Realized that `tokio::sync::Mutex` or `tokio::sync::RwLock` are not required as the struct is not held over an `await` point. Using `std::sync::Mutex` doesn't solve the issue.

## 2024-03-29
Ok, debug time. Just return a simple string instead of deserializing and processing the message. Surprise surprise, the benchmark holds.
Now only deserialize... BAM! Benchmark can't complete.

Read carefully the docs about `read_buf`: if the function returns `0`, either EOF has been reached (in our case, the client has closed the connection) or the buffer has a remaining capacity of `0`. Adding more logs over the buffer's len and capacity and also assigning a uuid to each thread to track the failures. Noticing that sometimes the message is incomplete, multiple times on the same thread. Noticing that the buffer is resized sometimes. Surprise surprise: the buffer must be cleared 🤦 Buffer size reduced from 4KB to 1KB.

Performance is good, even better than the original Redis, but while the original Redis consumes 80% of the CPU, our Redis consumes 280% of the CPU, at least according to `top`.

Next time:
- evaluate `std::sync::Mutex` over `tokio::sync::Mutex` for the performance side of things
- DON'T focus on CPU optimization for now
