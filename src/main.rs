use std::fs;
use std::io::{self, SeekFrom};
use std::iter::Zip;
use std::{
    cmp::min,
    fs::File,
    io::{Read, Seek},
    os::unix::fs::FileExt,
};
use flate2::read::DeflateDecoder;
use std::io::Write;
use std::path::Path;


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

#[derive(Debug)]
struct ZipFileEntry {
    filename: String,
    compressed_size: u32,
    uncompressed_size: u32,
    compression_method: u16,
    file_offset: u32,
}
fn main() {
    let res = read_end_central_dir("/home/abc/Desktop/example.zip");
    // println!("{:?}", res.unwrap());
    let entries = read_central_directory("/home/abc/Desktop/example.zip", res.unwrap()).unwrap();
    if let Some(entry) = entries.unwrap().get(0) {
        extract_file("/home/abc/Desktop/example.zip", entry);

    }
}

fn read_end_central_dir(path: &str) -> io::Result<Option<u64>> {
    let mut f = File::open(path)?;

    f.seek(SeekFrom::End(0))?;
    let file_size = f.stream_position()?;
    eprintln!("File size: {}", file_size);

    // as we need to check the last 1024
    let search_size = min(1024, file_size);
    f.seek(SeekFrom::End(-(search_size as i64)))?;
    let mut buf = vec![0; search_size as usize];
    f.read_exact(&mut buf)?;

    let signature_bytes: [u8; 4] = END_CENTRAL_DIR_SIGNATURE.to_le_bytes();
    eprintln!("Looking for signature: {:02X?}", signature_bytes);

    let mut signature_position: i64 = -1;

    for i in (0..buf.len().saturating_sub(4)).rev() {
        if buf[i..i + 4] == signature_bytes {
            signature_position = i as i64;
            break;
        }
    }

    if signature_position == -1 {
        eprintln!("Signature not found!");
        return Ok(None);
    }

    // Read the record bytes and print them before parsing
    let pos = signature_position as usize;
    // End of Central Directory Record:
    // [Signature (4 bytes)]
    // [Disk Number (2 bytes)]
    // [Start Disk (2 bytes)]
    // [Disk Entries (2 bytes)]
    // [Total Entries (2 bytes)]
    // [Directory Size (4 bytes)]
    // [Directory Offset (4 bytes)]
    // [Comment Length (2 bytes)]
    // [Optional Comment (variable)]
    let record_bytes = &buf[pos + 4..pos + 22]; // 18 bytes after signature
    eprintln!("Raw record bytes: {:02X?}", record_bytes);

    let end_central_dir = EndCentralDirectory {
        disk_num: u16::from_le_bytes(record_bytes[0..2].try_into().unwrap()),
        start_disk: u16::from_le_bytes(record_bytes[2..4].try_into().unwrap()),
        disk_entries: u16::from_le_bytes(record_bytes[4..6].try_into().unwrap()),
        total_entries: u16::from_le_bytes(record_bytes[6..8].try_into().unwrap()),
        dir_size: u32::from_le_bytes(record_bytes[8..12].try_into().unwrap()),
        dir_offset: u32::from_le_bytes(record_bytes[12..16].try_into().unwrap()),
        comment_len: u16::from_le_bytes(record_bytes[16..18].try_into().unwrap()),
    };

    Ok(Some(end_central_dir.dir_offset as u64))
}
fn read_central_directory(path: &str, offset: Option<u64>) -> io::Result<Option<Vec<ZipFileEntry>>> {
    // Central Directory Header:
    // [4 bytes]  Signature
    // [2 bytes]  Version made by
    // [2 bytes]  Version needed
    // [2 bytes]  General purpose bit flag
    // [2 bytes]  Compression method
    // [2 bytes]  Last modified time
    // [2 bytes]  Last modified date
    // [4 bytes]  CRC-32
    // [4 bytes]  Compressed size
    // [4 bytes]  Uncompressed size
    // [2 bytes]  Filename length
    // [2 bytes]  Extra field length
    // [2 bytes]  File comment length
    // [2 bytes]  Disk number start
    // [2 bytes]  Internal file attributes
    // [4 bytes]  External file attributes
    // [4 bytes]  Local header offset
    // [variable] Filename
    // [variable] Extra field
    // [variable] File comment
    let mut f: File = File::open(path)?;
    eprintln!("read_central_directory at offset: {}", offset.unwrap());

    // Read signature
    f.seek(SeekFrom::Start(offset.unwrap()))?;
    let mut buf = [0u8; 4];
    f.read_exact(&mut buf)?;
    if buf != CENTRAL_DIR_SIGNATURE.to_le_bytes() {
        eprintln!("No match on first read at offset {}", offset.unwrap());
        eprintln!(
            "Found {:02X?}, expected {:02X?}",
            buf,
            CENTRAL_DIR_SIGNATURE.to_le_bytes()
        );
        return Ok(None);
    }
    eprintln!("Found central directory signature!");

    // Skip version made by (2), version needed (2), flags (2)
    f.seek(SeekFrom::Current(6))?;

    // Read compression method
    let mut compression_method_buf = [0u8; 2];
    f.read_exact(&mut compression_method_buf)?;
    let compression_method = u16::from_le_bytes(compression_method_buf);
    eprintln!(
        "compression_method: {} (0x{:04X})",
        compression_method, compression_method
    );

    // Skip last mod time (2), last mod date (2), CRC32 (4)
    f.seek(SeekFrom::Current(8))?;

    // Read compressed and uncompressed sizes
    let mut compressions_buf = [0u8; 8];
    f.read_exact(&mut compressions_buf)?;
    let compressed_size = u32::from_le_bytes(compressions_buf[0..4].try_into().unwrap());
    let uncompressed_size = u32::from_le_bytes(compressions_buf[4..8].try_into().unwrap());
    eprintln!(
        "compressed {} uncompressed {}",
        compressed_size, uncompressed_size
    );

    // Read filename length, extra field length, comment length
    let mut lengths_buf = [0u8; 6];
    f.read_exact(&mut lengths_buf)?;
    let filename_length = u16::from_le_bytes(lengths_buf[0..2].try_into().unwrap());
    let extra_length = u16::from_le_bytes(lengths_buf[2..4].try_into().unwrap());
    let comment_length = u16::from_le_bytes(lengths_buf[4..6].try_into().unwrap());
    eprintln!(
        "filename_length {} extra_length {} comment_length {}",
        filename_length, extra_length, comment_length
    );

    // Skip disk number start (2), internal attrs (2), external attrs (4)
    f.seek(SeekFrom::Current(8))?;

    // Read local header offset
    let mut offset_buf = [0u8; 4];
    f.read_exact(&mut offset_buf)?;
    let file_offset = u32::from_le_bytes(offset_buf);
    eprintln!("file_offset: {}", file_offset);

    // Read filename
    let mut filename_buf = vec![0u8; filename_length as usize];
    f.read_exact(&mut filename_buf)?;
    let filename = String::from_utf8(filename_buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    eprintln!("filename: {}", filename);

    let mut file_entries: Vec<ZipFileEntry> = vec![];

    file_entries.push(ZipFileEntry {
        filename: filename,
        compressed_size: compressed_size,
        uncompressed_size: uncompressed_size,
        compression_method: compression_method,
        file_offset: file_offset,
    });

    eprintln!("file_entries: {:?}", file_entries);

    // Skip extra field
    f.seek(SeekFrom::Current(extra_length as i64))?;

    // Skip comment
    f.seek(SeekFrom::Current(comment_length as i64))?;

    Ok(Some(file_entries))
}

fn extract_file(path: &str, entry: &ZipFileEntry)-> io::Result<Option<Vec<u8>>> {
    eprintln!("Starting extract_file with:");
    eprintln!("  filename: {}", entry.filename);
    eprintln!("  file_offset: {}", entry.file_offset);
    eprintln!("  compressed_size: {}", entry.compressed_size);
    eprintln!("  uncompressed_size: {}", entry.uncompressed_size);
    eprintln!("  compression_method: {}", entry.compression_method);
    
    let mut f: File = File::open(path)?;
    f.seek(SeekFrom::Start(entry.file_offset as u64))?;

    // Read and verify local file header
    let mut local_header = [0u8; 30];
    f.read_exact(&mut local_header)?;
    eprintln!("Local header: {:02X?}", local_header);
    
    // Check signature
    if local_header[0..4] != LOCAL_FILE_HEADER_SIGNATURE.to_le_bytes() {
        eprintln!("Invalid local file header signature");
        return Ok(None);
    }

    let local_name_length = u16::from_le_bytes(local_header[26..28].try_into().unwrap());
    let local_extra_length = u16::from_le_bytes(local_header[28..30].try_into().unwrap());
    eprintln!("Local header name length: {}", local_name_length);
    eprintln!("Local header extra length: {}", local_extra_length);

    // Skip variable length fields
    f.seek(SeekFrom::Current((local_name_length + local_extra_length) as i64))?;

    // Read compressed data
    let mut compressed_data_buf = vec![0u8; entry.compressed_size as usize];
    f.read_exact(&mut compressed_data_buf)?;
    eprintln!("Read {} bytes of compressed data", compressed_data_buf.len());

    match entry.compression_method {
        0 => {
            eprintln!("No compression, returning raw data");
            Ok(Some(compressed_data_buf))
        },
        8 => {
            eprintln!("Using Deflate decompression");
            eprintln!("Compressed size: {}", compressed_data_buf.len());
            eprintln!("First few bytes: {:02X?}", &compressed_data_buf[..16.min(compressed_data_buf.len())]);

            use flate2::read::DeflateDecoder;
            use std::io::Read;

            let mut decoder = DeflateDecoder::new(&compressed_data_buf[..]);
            let mut decompressed_data = Vec::with_capacity(entry.uncompressed_size as usize);
            
            match decoder.read_to_end(&mut decompressed_data) {
                Ok(size) => {
                    eprintln!("Successfully decompressed {} bytes", size);
                    if size != entry.uncompressed_size as usize {
                        eprintln!("Warning: Decompressed size {} differs from expected {}", 
                                size, entry.uncompressed_size);
                    }
                    eprintln!("First few bytes of decompressed data: {:02X?}", 
                            &decompressed_data[..16.min(decompressed_data.len())]);
                    let path = Path::new("/home/abc/Desktop/vscode2.txt");
                    let mut file = File::create(path)?;
                    file.write_all(&decompressed_data)?;
                    file.flush()?;

                    Ok(Some(decompressed_data))
                },
                Err(e) => {
                    eprintln!("Decompression error: {}", e);
                    eprintln!("Full compressed data: {:02X?}", compressed_data_buf);
                    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                }
            }
        },
        _ => {
            eprintln!("Unsupported compression method: {}", entry.compression_method);
            Ok(None)
        }
    }
}

