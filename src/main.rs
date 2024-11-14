use clap::Parser;
use std::io::Write;
use std::io::{self, SeekFrom};
use std::path::Path;
use std::{
    cmp::min,
    fs::File,
    io::{Read, Seek},
};

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long)]
    archive_path: String,

    #[arg(short, long)]
    path_to_unpack: String,
}

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
    let args = Args::parse();
    let archive_path: &str = &args.archive_path;
    let path_to_unpack: &str = &args.path_to_unpack;
    let res: Result<Option<u64>, io::Error> = read_end_central_dir(archive_path);
    let entries: Option<Vec<ZipFileEntry>> =
        read_central_directory(archive_path, res.unwrap()).unwrap();
    if let Some(entries_vec) = &entries {
        for item in entries_vec {
            extract_file(archive_path, item, path_to_unpack);
        }
    }
}

fn read_end_central_dir(path: &str) -> io::Result<Option<u64>> {
    let mut f: File = File::open(path)?;

    f.seek(SeekFrom::End(0))?;
    let file_size: u64 = f.stream_position()?;
    eprintln!("File size: {} bytes", file_size);

    // as we need to check the last 1024
    let search_size: u64 = min(1024, file_size);
    f.seek(SeekFrom::End(-(search_size as i64)))?;
    let mut buf: Vec<u8> = vec![0; search_size as usize];
    f.read_exact(&mut buf)?;

    let signature_bytes: [u8; 4] = END_CENTRAL_DIR_SIGNATURE.to_le_bytes();

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
    let pos: usize = signature_position as usize;
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
    let record_bytes: &[u8] = &buf[pos + 4..pos + 22]; // 18 bytes after signature

    let end_central_dir: EndCentralDirectory = EndCentralDirectory {
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
fn read_central_directory(
    path: &str,
    offset: Option<u64>,
) -> io::Result<Option<Vec<ZipFileEntry>>> {
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
    let mut file_entries: Vec<ZipFileEntry> = vec![];
    let mut current_offset: u64 = offset.unwrap();

    loop {
        f.seek(SeekFrom::Start(current_offset))?;

        // Read signature
        let mut buf: [u8; 4] = [0u8; 4];
        match f.read_exact(&mut buf) {
            Ok(_) => {
                if buf != CENTRAL_DIR_SIGNATURE.to_le_bytes() {
                    break;
                }
            }
            Err(_) => break,
        }

        // Skip version made by (2), version needed (2), flags (2)
        f.seek(SeekFrom::Current(6))?;

        // Read compression method
        let mut compression_method_buf = [0u8; 2];
        f.read_exact(&mut compression_method_buf)?;
        let compression_method = u16::from_le_bytes(compression_method_buf);

        // Skip last mod time (2), last mod date (2), CRC32 (4)
        f.seek(SeekFrom::Current(8))?;

        // Read sizes
        let mut compressions_buf: [u8; 8] = [0u8; 8];
        f.read_exact(&mut compressions_buf)?;
        let compressed_size: u32 = u32::from_le_bytes(compressions_buf[0..4].try_into().unwrap());
        let uncompressed_size: u32 = u32::from_le_bytes(compressions_buf[4..8].try_into().unwrap());

        // Read lengths
        let mut lengths_buf: [u8; 6] = [0u8; 6];
        f.read_exact(&mut lengths_buf)?;
        let filename_length: u16 = u16::from_le_bytes(lengths_buf[0..2].try_into().unwrap());
        let extra_length: u16 = u16::from_le_bytes(lengths_buf[2..4].try_into().unwrap());
        let comment_length: u16 = u16::from_le_bytes(lengths_buf[4..6].try_into().unwrap());

        // Skip to local header offset
        f.seek(SeekFrom::Current(8))?;

        // Read local header offset
        let mut offset_buf: [u8; 4] = [0u8; 4];
        f.read_exact(&mut offset_buf)?;
        let file_offset: u32 = u32::from_le_bytes(offset_buf);

        // Read filename
        let mut filename_buf: Vec<u8> = vec![0u8; filename_length as usize];
        f.read_exact(&mut filename_buf)?;
        let filename: String =
            String::from_utf8(filename_buf).map_err(|e: std::string::FromUtf8Error| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
            })?;

        file_entries.push(ZipFileEntry {
            filename,
            compressed_size,
            uncompressed_size,
            compression_method,
            file_offset,
        });
        // Skip extra field and comment
        f.seek(SeekFrom::Current((extra_length + comment_length) as i64))?;
        current_offset = f.stream_position()?;
    }
    eprintln!("file_entries: {:?}", file_entries);

    Ok(Some(file_entries))
}

fn extract_file(
    path: &str,
    entry: &ZipFileEntry,
    path_to_unpack: &str,
) -> io::Result<Option<Vec<u8>>> {
    eprintln!("Starting extract_file with:");
    eprintln!("  filename: {}", entry.filename);

    let mut f: File = File::open(path)?;
    f.seek(SeekFrom::Start(entry.file_offset as u64))?;

    // Read and verify local file header
    let mut local_header: [u8; 30] = [0u8; 30];
    f.read_exact(&mut local_header)?;

    // Check signature
    if local_header[0..4] != LOCAL_FILE_HEADER_SIGNATURE.to_le_bytes() {
        eprintln!("Invalid local file header signature");
        return Ok(None);
    }

    let local_name_length: u16 = u16::from_le_bytes(local_header[26..28].try_into().unwrap());
    let local_extra_length: u16 = u16::from_le_bytes(local_header[28..30].try_into().unwrap());

    // Skip variable length fields
    f.seek(SeekFrom::Current(
        (local_name_length + local_extra_length) as i64,
    ))?;

    // Read compressed data
    let mut compressed_data_buf: Vec<u8> = vec![0u8; entry.compressed_size as usize];
    f.read_exact(&mut compressed_data_buf)?;

    match entry.compression_method {
        0 => {
            eprintln!("No compression, returning raw data");
            Ok(Some(compressed_data_buf))
        }
        8 => {
            eprintln!("Using Deflate decompression");
            eprintln!("Compressed size: {}", compressed_data_buf.len());

            use flate2::read::DeflateDecoder;
            use std::io::Read;

            let mut decoder: DeflateDecoder<&[u8]> = DeflateDecoder::new(&compressed_data_buf[..]);
            let mut decompressed_data: Vec<u8> =
                Vec::with_capacity(entry.uncompressed_size as usize);

            match decoder.read_to_end(&mut decompressed_data) {
                Ok(size) => {
                    eprintln!("Successfully decompressed {} bytes", size);
                    if size != entry.uncompressed_size as usize {
                        eprintln!(
                            "Warning: Decompressed size {} differs from expected {}",
                            size, entry.uncompressed_size
                        );
                    }
                    let full_path: String = format!("{}{}", path_to_unpack, entry.filename);
                    let path: &Path = Path::new(&full_path);
                    let mut file: File = File::create(path)?;
                    file.write_all(&decompressed_data)?;
                    file.flush()?;

                    eprintln!("Successfully saved file to {}", full_path);

                    Ok(Some(decompressed_data))
                }
                Err(e) => {
                    eprintln!("Decompression error: {}", e);
                    eprintln!("Full compressed data: {:02X?}", compressed_data_buf);
                    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                }
            }
        }
        _ => {
            eprintln!(
                "Unsupported compression method: {}",
                entry.compression_method
            );
            Ok(None)
        }
    }
}
