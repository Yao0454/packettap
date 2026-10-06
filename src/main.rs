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
    bandwidth: u64, // bytes/second
    start: Instant,
    total_bytes: u64,
}

impl RateLimiter {
    fn new(bandwidth: u64) -> Self {
        Self {
            bandwidth,
            start: Instant::now(),
            total_bytes: 0,
        }
    }

    fn consume(&mut self, bytes: usize) {
        self.total_bytes += bytes as u64;

        let expected_secs = self.total_bytes as f64 / self.bandwidth as f64;

        let expected = Duration::from_secs_f64(expected_secs);

        let elapsed = self.start.elapsed();

        if expected > elapsed {
            sleep(expected - elapsed);
        }
    }
}

fn dump(data: &[u8]) {
    for (offset, chunk) in data.chunks(16).enumerate() {
        print!("{:04x} ", offset * 16);

        for i in 0..16 {
            if i < chunk.len() {
                print!("{:02x} ", chunk[i]);
            } else {
                print!("  ");
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

fn forword(
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

        if hex {
            dump(&buf[..n]);
            println!("[conn {conn_id}] [{direction}] {n} bytes");
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
        forword(
            client_read,
            server,
            conn_id,
            "Client->Server",
            client_to_server_config,
            hex,
        )
    });

    let server_to_client_result = forword(
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

        println!("client connected: {addr}");

        spawn(move || {
            if let Err(e) = handle_client(client, conn_id, config) {
                eprintln!("connection error: {e}");
            }
        }); // 独立线程
    }
}
