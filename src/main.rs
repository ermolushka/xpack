use std::io::{self};
use std::{
    cmp::min,
    fs::File,
    io::{Read, Seek},
    os::unix::fs::FileExt,
};

const LOCAL_FILE_HEADER_SIGNATURE: i32 = 0x04034b50;
const CENTRAL_DIR_SIGNATURE: i32 = 0x02014b50;
const END_CENTRAL_DIR_SIGNATURE: i32 = 0x06054b50;

struct EndCentralDirectory {
    disk_num: u16,
    start_disk: u16,
    disk_entries: u16,
    total_entries: u16,
    dir_size: u32,
    dir_offset: u32,
    comment_len: u16,
}
fn main() {
    let res = read_end_central_dir("/home/abc/Desktop/example.zip");
    println!("{:?}", res.unwrap());
}

fn read_end_central_dir(path: &str) -> io::Result<Option<u64>> {
    let mut f = File::open(path)?;

    // move to the end of file and get current position
    f.seek(std::io::SeekFrom::End(0))?;
    let file_size = f.stream_position()?;

    // Look for the last 1KB or the whole file if smaller as per ZIP specs
    // seek to the search area

    let search_size = min(1024, file_size);
    f.seek(std::io::SeekFrom::End(-(search_size as i64)))?; // should be negative to seek from end
    let mut buf = vec![0; search_size as usize];
    f.read_to_end(&mut buf)?; // better than read_exact as read_exact will fail if can't fill the whole buf

    let signature_bytes: [u8; 4] = END_CENTRAL_DIR_SIGNATURE.to_le_bytes();

    let mut signature_position: i64 = 0;

    for i in (0..buf.len().saturating_sub(4)).rev() {
        if buf[i..i + 4] == signature_bytes {
            signature_position = i as i64;
        }
    }

    f.seek(std::io::SeekFrom::End(
        -(-(search_size as i64) + signature_position + 4 as i64),
    ))?; // 4 for skipping the signature itself

    let mut record = [0u8; 18];
    f.read_exact(&mut record)?;

    let end_central_dir = EndCentralDirectory {
        disk_num: u16::from_le_bytes(record[0..2].try_into().unwrap()),
        start_disk: u16::from_le_bytes(record[2..4].try_into().unwrap()),
        disk_entries: u16::from_le_bytes(record[4..6].try_into().unwrap()),
        total_entries: u16::from_le_bytes(record[6..8].try_into().unwrap()),
        dir_size: u32::from_le_bytes(record[8..12].try_into().unwrap()),
        dir_offset: u32::from_le_bytes(record[12..16].try_into().unwrap()),
        comment_len: u16::from_le_bytes(record[16..18].try_into().unwrap()),
    };
    Ok(Some(end_central_dir.dir_offset as u64))
}
