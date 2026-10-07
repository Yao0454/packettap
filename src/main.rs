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

use clap::Parser;
use rand::RngExt;

static NEXT_CONN_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy)]
struct DirectionConfig {
    latency: Duration,
    jitter: Duration,

    bandwidth: Option<u64>, // bytes/second
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    listen: String,

    #[arg(long)]
    upstream: String,

    #[arg(long, default_value_t = 0)]
    client_to_server_latency: u64,

    #[arg(long, default_value_t = 0)]
    client_to_server_jitter: u64,

    #[arg(long)]
    client_to_server_bandwidth: Option<u64>,

    #[arg(long)]
    server_to_client_bandwidth: Option<u64>,

    #[arg(long, default_value_t = 0)]
    server_to_client_latency: u64,

    #[arg(long, default_value_t = 0)]
    server_to_client_jitter: u64,

    #[arg(long)]
    hex: bool,
}

#[derive(Debug)]
struct Config {
    upstream: String,
    client_to_server: DirectionConfig,
    server_to_client: DirectionConfig,
    hex: bool,
}

struct RateLimiter {
    rate: f64, // bytes/second
    capacity: f64,
    tokens: f64,
    last_update: Instant,
}

impl RateLimiter {
    fn new(bandwidth: u64) -> Self {
        assert!(bandwidth > 0);

        let capacity = bandwidth as f64 * 0.1;
        Self {
            rate: bandwidth as f64,
            capacity,
            tokens: capacity,
            last_update: Instant::now(),
        }
    }

    fn refill_by(&mut self, elapsed: Duration) {
        self.tokens = (self.tokens + elapsed.as_secs_f64() * self.rate).min(self.capacity);
    }

    fn refill(&mut self) {
        let now = Instant::now();

        let elapsed = now.duration_since(self.last_update);

        self.refill_by(elapsed);
        self.last_update = now;
    }

    fn consume(&mut self, bytes: usize) {
        self.refill();

        let required = bytes as f64;

        if self.tokens >= required {
            self.tokens -= required;
            return;
        }

        let missing = required - self.tokens;
        let wait_secs = missing / self.rate;

        sleep(Duration::from_secs_f64(wait_secs));

        self.tokens = 0.0;
        self.last_update = Instant::now();
    }
}

fn dump(data: &[u8]) {
    for (offset, chunk) in data.chunks(16).enumerate() {
        print!("{:04x} ", offset * 16);

        for i in 0..16 {
            if i < chunk.len() {
                print!("{:02x} ", chunk[i]);
            } else {
                print!("   ");
            }
        }

        print!(" ");

        for &byte in chunk {
            if byte.is_ascii_graphic() || byte == b' ' {
                print!("{}", byte as char);
            } else {
                print!(".");
            }
        }
        println!();
    }
}

fn apply_delay(base: Duration, jitter: Duration) {
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

// fn apply_bandwidth_limit(bytes: usize, bandwidth: Option<u64>) {
//     let Some(bytes_per_sec) = bandwidth else {
//         return;
//     };

//     if bytes_per_sec == 0 {
//         return;
//     }

//     let seconds = bytes as f64 / bytes_per_sec as f64;

//     sleep(Duration::from_secs_f64(seconds));
// }

fn forward(
    mut reader: TcpStream,
    mut writer: TcpStream,
    conn_id: u64,
    direction: &str,
    direction_config: DirectionConfig,
    hex: bool,
) -> io::Result<u64> {
    let mut limiter = direction_config.bandwidth.map(RateLimiter::new);

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
            dump(&buf[..n]);
        }

        apply_delay(direction_config.latency, direction_config.jitter);
        // apply_bandwidth_limit(n, direction_config.bandwidth);

        if let Some(limiter) = &mut limiter {
            limiter.consume(n);
        }
        writer.write_all(&buf[..n])?;
        total_bytes += n as u64;
    }

    Ok(total_bytes)
}

fn handle_client(client: TcpStream, conn_id: u64, config: Arc<Config>) -> io::Result<()> {
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

fn main() -> io::Result<()> {
    let args = Args::parse();

    // Arc stands for Atomic Reference Counted
    let config = Arc::new(Config {
        upstream: args.upstream,
        client_to_server: DirectionConfig {
            latency: Duration::from_millis(args.client_to_server_latency),
            jitter: Duration::from_millis(args.client_to_server_jitter),
            bandwidth: args.client_to_server_bandwidth,
        },
        server_to_client: DirectionConfig {
            latency: Duration::from_millis(args.server_to_client_latency),
            jitter: Duration::from_millis(args.server_to_client_jitter),
            bandwidth: args.server_to_client_bandwidth,
        },
        hex: args.hex,
    });

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_bucket_starts_full() {
        let limiter = RateLimiter::new(1000);

        assert_eq!(limiter.rate, 1000.0);
        assert_eq!(limiter.capacity, 100.0);
        assert_eq!(limiter.tokens, 100.0);
    }

    #[test]
    fn refill_adds_tokens() {
        let mut limiter = RateLimiter::new(1000);
        limiter.tokens = 0.0;
        limiter.refill_by(Duration::from_millis(50));

        assert_eq!(limiter.tokens, 50.0);
    }

    #[test]
    fn refill_does_not_excced_capacity() {
        let mut limiter = RateLimiter::new(1000);

        limiter.tokens = 90.0;
        limiter.refill_by(Duration::from_millis(100));
        assert_eq!(limiter.tokens, 100.0);
    }
}
