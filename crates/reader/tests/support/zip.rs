#[derive(Clone, Copy)]
pub enum Packing {
    Stored,
    Deflated,
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

pub fn zip(entries: &[(&str, Packing, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut directory = Vec::new();
    for &(name, packing, data) in entries {
        let (method, packed) = match packing {
            Packing::Stored => (0u16, data.to_vec()),
            Packing::Deflated => (8u16, miniz_oxide::deflate::compress_to_vec(data, 6)),
        };
        let local = out.len() as u32;
        let crc = crc32(data);
        let shared = |into: &mut Vec<u8>| {
            into.extend_from_slice(&20u16.to_le_bytes());
            into.extend_from_slice(&0u16.to_le_bytes());
            into.extend_from_slice(&method.to_le_bytes());
            into.extend_from_slice(&[0; 4]);
            into.extend_from_slice(&crc.to_le_bytes());
            into.extend_from_slice(&(packed.len() as u32).to_le_bytes());
            into.extend_from_slice(&(data.len() as u32).to_le_bytes());
            into.extend_from_slice(&(name.len() as u16).to_le_bytes());
            into.extend_from_slice(&0u16.to_le_bytes());
        };
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        shared(&mut out);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&packed);
        directory.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        directory.extend_from_slice(&20u16.to_le_bytes());
        shared(&mut directory);
        directory.extend_from_slice(&[0; 10]);
        directory.extend_from_slice(&local.to_le_bytes());
        directory.extend_from_slice(name.as_bytes());
    }
    let start = out.len() as u32;
    out.extend_from_slice(&directory);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(directory.len() as u32).to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}
