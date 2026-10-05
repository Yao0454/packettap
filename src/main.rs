use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    thread::spawn,
};

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

fn forword(mut reader: TcpStream, mut writer: TcpStream, direction: &str) -> io::Result<()> {
    let mut buf = [0u8; 4096];

    loop {
        let n = reader.read(&mut buf)?;

        if n == 0 {
            writer.shutdown(std::net::Shutdown::Write)?;
            break;
        }

        println!("[{direction}] {n} bytes");
        dump(&buf[..n]);

        writer.write_all(&buf[..n])?;
    }

    Ok(())
}

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:9000")?;

    println!("PacketTap listening on 127.0.0.1:9000");

    let (client, client_addr) = listener.accept()?;

    println!("client connected: {client_addr}");

    let server = TcpStream::connect("127.0.0.1:8000")?;

    println!("connected to upstream server");

    let client_read = client.try_clone()?;
    let server_read = server.try_clone()?;

    let t1 = spawn(move || forword(client_read, server, "Client -> Server"));
    let t2 = spawn(move || forword(server_read, client, "Server -> Client"));

    t1.join().unwrap()?;
    t2.join().unwrap()?;

    Ok(())
}
