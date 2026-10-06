use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::atomic::{AtomicU64, Ordering},
    thread::{sleep, spawn},
    time::{Duration, Instant},
};

use clap::Parser;
use rand::{Rng, RngExt};

static NEXT_CONN_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    listen: String,

    #[arg(long)]
    upstream: String,

    #[arg(long, default_value_t = 0)]
    latency: u64,

    #[arg(long, default_value_t = 0)]
    jitter: u64,
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

fn apply_delay(base_ms: u64, jitter_ms: u64) -> u64 {
    let mut rng = rand::rng();

    let min = base_ms.saturating_sub(jitter_ms); // 防止向下溢出
    let max = base_ms + jitter_ms;

    let delay = rng.random_range(min..=max);

    sleep(Duration::from_millis(delay));

    delay
}

fn forword(
    mut reader: TcpStream,
    mut writer: TcpStream,
    conn_id: u64,
    direction: &str,
) -> io::Result<u64> {
    let mut buf = [0u8; 4096];
    let mut total_bytes = 0u64;

    loop {
        let n = reader.read(&mut buf)?;

        if n == 0 {
            writer.shutdown(std::net::Shutdown::Write)?;
            break;
        }

        println!("[conn {conn_id}] [{direction}] {n} bytes");
        dump(&buf[..n]);

        let delay = apply_delay(100, 20);
        println!("[conn {conn_id}] [{direction}] delayed {delay} ms");

        writer.write_all(&buf[..n])?;
        total_bytes += n as u64;
    }

    Ok(total_bytes)
}

fn handle_client(client: TcpStream, conn_id: u64) -> io::Result<()> {
    let start = Instant::now();
    let server = TcpStream::connect("127.0.0.1:8000")?;

    println!("connected to upstream server");

    let client_read = client.try_clone()?;
    let server_read = server.try_clone()?;

    let c2s = spawn(move || forword(client_read, server, conn_id, "Client -> Server"));

    let s2c_result = forword(server_read, client, conn_id, "Server -> Client");

    let c2s_result = c2s.join().unwrap();

    let s2c_bytes = s2c_result?;
    let c2s_bytes = c2s_result?;

    let elapsed = start.elapsed();
    println!(
        "[conn {conn_id}] closed: Client->Server={c2s_bytes} bytes, Server->Client={s2c_bytes} bytes, duration={elapsed:?}"
    );

    Ok(())
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let addr = &args.listen;
    let listener = TcpListener::bind(addr)?;

    println!("PacketTap listening on {addr}");

    loop {
        let (client, addr) = listener.accept()?;

        let conn_id = NEXT_CONN_ID.fetch_add(1, Ordering::Relaxed);

        println!("client connected: {addr}");

        spawn(move || {
            if let Err(e) = handle_client(client, conn_id) {
                eprintln!("connection error: {e}");
            }
        }); // 独立线程
    }
}
