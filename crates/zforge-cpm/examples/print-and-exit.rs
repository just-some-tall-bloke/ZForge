use std::io;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let image = [
        0x11, 0x0b, 0x01, // LD DE,010Bh
        0x0e, 0x09, // LD C,9
        0xcd, 0x05, 0x00, // CALL 0005h
        0xc3, 0x00, 0x00, // JP 0000h
        b'Z', b'F', b'o', b'r', b'g', b'e', b' ', b'C', b'P', b'/', b'M', b'\r', b'\n', b'$',
    ];
    zforge_cpm::run(&image, &mut io::stdout().lock(), 16)?;
    Ok(())
}
