`flatten_iters` flattens a stream of iterators into one continuous stream.

This is useful when you have a producer that is paging through a resource (like a REST endpoint
with pages or a next URL, an ElasticSearch query with a scroll parameter, etc.)

This code is taken *almost* verbatim from [`StreamExt::flatten`] and is similar
in spirit to [`Iterator::flatten`].

```rust
use stream_flatten_iters::StreamExt as _;
use futures::stream::StreamExt;

#[tokio::main]
async fn main() {
    let (mut tx, mut rx) = tokio::sync::mpsc::channel(3);

    tokio::spawn(async move {
        tx.send(vec![0, 1, 2, 3]).await.unwrap();
        tx.send(vec![4, 5, 6]).await.unwrap();
        tx.send(vec![7, 8, 9]).await.unwrap();
    });

    let mut stream = rx.flatten_iters();

    while let Some(res) = stream.next().await {
        println!("got = {}", res);
    }
}

// Output:
// got = 0
// got = 1
// got = 2
// got = 3
// got = 4
// got = 5
// got = 6
// got = 7
// got = 8
// got = 9
```

This is especially useful when combined with [`StreamExt::buffered`] to keep a buffer of promises going
throughout a long promise.

```rust
use stream_flatten_iters::StreamExt as _;
use futures::stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut tx, mut rx) = tokio::sync::mpsc::channel(3);

    tokio::spawn(async move {
        for i in 0_usize..100 {
            let start = i * 10;
            let end = start + 10;
            tx.send(start..end).await.unwrap();
        }
    });

    let mut stream = rx.flatten_iters().map(|i| long_process(i)).buffered(10);

    let mut total = 0_usize;
    while let Some(res) = stream.next().await {
        let _ = res?;
        total += 1;
        println!("Completed {} tasks", total);
    }

    Ok(())
}

async fn long_process(i: usize) -> Result<(), Box<dyn std::error::Error>> {
    // Do something that takes a long time
    Ok(())
}
```

[`StreamExt::flatten`]: https://docs.rs/futures/0.3/futures/stream/trait.StreamExt.html#method.flatten
[`Iterator::flatten`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.flatten
[`StreamExt::buffered`]: https://docs.rs/futures/0.3/futures/stream/trait.StreamExt.html#method.buffered
