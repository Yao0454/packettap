use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::atomic::{AtomicU64, Ordering},
    thread::spawn,
};

static NEXT_CONN_ID: AtomicU64 = AtomicU64::new(1);

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

fn forword(
    mut reader: TcpStream,
    mut writer: TcpStream,
    conn_id: u64,
    direction: &str,
) -> io::Result<()> {
    let mut buf = [0u8; 4096];

    loop {
        let n = reader.read(&mut buf)?;

        if n == 0 {
            writer.shutdown(std::net::Shutdown::Write)?;
            break;
        }

        println!("[conn {conn_id}] [{direction}] {n} bytes");
        dump(&buf[..n]);

        writer.write_all(&buf[..n])?;
    }

    Ok(())
}

fn handle_client(client: TcpStream, conn_id: u64) -> io::Result<()> {
    let server = TcpStream::connect("127.0.0.1:8000")?;

    println!("connected to upstream server");

    let client_read = client.try_clone()?;
    let server_read = server.try_clone()?;

    let c2s = spawn(move || forword(client_read, server, conn_id, "Client -> Server"));
    forword(server_read, client, conn_id, "Server -> Client")?;

    c2s.join().unwrap()?;

    Ok(())
}

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:9000")?;

    println!("PacketTap listening on 127.0.0.1:9000");

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
