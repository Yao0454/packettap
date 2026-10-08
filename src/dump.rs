pub fn dump(data: &[u8]) {
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
