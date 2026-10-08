use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread::{sleep, spawn},
    time::{Duration, Instant},
};

use rand::RngExt;

use crate::{
    config::{Args, Config, DirectionConfig},
    dump,
    rate_limiter::RateLimiter,
};

static NEXT_CONN_ID: AtomicU64 = AtomicU64::new(1);

pub fn apply_delay(base: Duration, jitter: Duration) {
    if base.is_zero() && jitter.is_zero() {
        return;
    }

    let base_ms = base.as_millis() as u64;
    let jitter_ms = jitter.as_millis() as u64;

    let min = base_ms.saturating_sub(jitter_ms); // 防止向下溢出
    let max = base_ms + jitter_ms;

    let mut rng = rand::rng();
    let delay_ms = rng.random_range(min..=max);

    sleep(Duration::from_millis(delay_ms));
}

pub fn forward(
    mut reader: TcpStream,
    mut writer: TcpStream,
    conn_id: u64,
    direction: &str,
    direction_config: DirectionConfig,
    hex: bool,
) -> io::Result<u64> {
    let mut limiter = match (direction_config.bandwidth, direction_config.burst) {
        (Some(bandwidth), Some(burst)) => Some(RateLimiter::new(bandwidth, burst)),

        (Some(bandwidth), None) => {
            let burst = (bandwidth / 10).max(1);
            Some(RateLimiter::new(bandwidth, burst))
        }

        (None, _) => None,
    };

    let mut buf = [0u8; 4096];
    let mut total_bytes = 0u64;

    loop {
        let n = reader.read(&mut buf)?;

        if n == 0 {
            writer.shutdown(std::net::Shutdown::Write)?;
            break;
        }

        println!("[conn {conn_id}] [{direction}] {n} bytes");

        if hex {
            dump::dump(&buf[..n]);
        }

        apply_delay(direction_config.latency, direction_config.jitter);
        // apply_bandwidth_limit(n, direction_config.bandwidth);

        if let Some(limiter) = &mut limiter {
            let max_chunk_size = limiter.max_chunk_size();

            for chunk in buf[..n].chunks(max_chunk_size) {
                limiter.consume(chunk.len());
                writer.write_all(chunk)?;
            }
        } else {
            writer.write_all(&buf[..n])?;
        }
        total_bytes += n as u64;
    }

    Ok(total_bytes)
}

pub fn handle_client(client: TcpStream, conn_id: u64, config: Arc<Config>) -> io::Result<()> {
    let start = Instant::now();
    let server = TcpStream::connect(&config.upstream)?;

    println!("connected to upstream server");

    let client_read = client.try_clone()?;
    let server_read = server.try_clone()?;

    let client_to_server_config = config.client_to_server;
    let server_to_client_config = config.server_to_client;

    let hex = config.hex;

    let client_to_server = spawn(move || {
        forward(
            client_read,
            server,
            conn_id,
            "Client->Server",
            client_to_server_config,
            hex,
        )
    });

    let server_to_client_result = forward(
        server_read,
        client,
        conn_id,
        "Server->Client",
        server_to_client_config,
        hex,
    );

    let client_to_server_result = client_to_server.join().unwrap();

    let server_to_client_bytes = server_to_client_result?;
    let client_to_server_bytes = client_to_server_result?;

    let elapsed = start.elapsed();
    println!(
        "[conn {conn_id}] closed: Client->Server={client_to_server_bytes} bytes, Server->Client={server_to_client_bytes} bytes, duration={elapsed:?}"
    );

    Ok(())
}

pub fn run(args: Args, config: Arc<Config>) -> io::Result<()> {
    let addr = &args.listen;
    let listener = TcpListener::bind(addr)?;
    println!("PacketTap listening on {addr}");

    loop {
        let (client, addr) = listener.accept()?;

        let conn_id = NEXT_CONN_ID.fetch_add(1, Ordering::Relaxed);

        let config = Arc::clone(&config);

        println!("[conn {conn_id}] client connected: {addr}");

        spawn(move || {
            if let Err(e) = handle_client(client, conn_id, config) {
                eprintln!("connection error: {e}");
            }
        }); // 独立线程
    }
}
