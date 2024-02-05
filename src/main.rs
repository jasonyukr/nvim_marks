use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::str;
use std::env;
use std::io::{Read, Result};

const DEBUG: bool = false;

fn read_fully<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<()> {
    let mut bytes_read = 0;

    while bytes_read < buf.len() {
        match reader.read(&mut buf[bytes_read..]) {
            Ok(0) => return Err(std::io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => bytes_read += n,
            Err(err) => return Err(err),
        }
    }

    Ok(())
}

fn read_uint<R: BufRead>(reader: &mut R) -> (usize, usize) {
    let mut len = [0u8];
    let mut len1 = [0u8];
    let mut len2 = [0u8; 2];
    let mut len4 = [0u8; 4];
    let length : usize;

    read_fully(reader, &mut len).expect("len");
    match len[0] {
        0xCC => {
            read_fully(reader, &mut len1).expect("len_");
            length = len1[0] as usize;
            return (length, 2);
        },
        0xCD => {
            read_fully(reader, &mut len2).expect("len2");
            length = ((len2[0] as usize) << 8) +
                (len2[1] as usize);
            return (length, 3);
        },
        0xCE => {
            read_fully(reader, &mut len4).expect("len4");
            length = ((len4[0] as usize) << 24) +
                ((len4[1] as usize) << 16) +
                ((len4[2] as usize) << 8) +
                (len4[3] as usize);
            return (length, 5);
        },
        0xCF => {
            // uint64 case
            // I don't expect this case in real life !!
            panic!("uint64 case");
        },
        0xD0|0xD1|0xD2|0xD3 => {
            // signed int case
            panic!("signed int case");
        },
        _ => {
            length = len[0] as usize;
            if length > 0xE0 {
                panic!("signed int case");
            }
            return (length, 1);
        },
    }
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let file: File;
    if let Some(path) = env::home_dir() {
        let mut filename = format!("{}/{}", path.display(), ".local/state/nvim/shada/main.shada");
        if args.len() == 2 {
            filename = args[1].clone();
        }
        file = match File::open(filename) {
            Err(_) => panic!("can't open file"),
            Ok(file) => file,
        };
    } else {
        panic!("can't get home_dir");
    }
    let mut reader = BufReader::new(file);

    let mut cnt = 1;
    loop {
        let mut entry_type = [0u8; 2];
        let mut timestamp = [0u8; 4];
        let mut tag = [0u8];
        let mut key = [0u8];
        let total_length;
        let mut length;
        let mut consumed;
        let mut processed : usize;

        match read_fully(&mut reader, &mut entry_type) {
            Ok(()) => {},
            Err(_) => break, // we expect EOF here
        }

        if DEBUG {
            cnt = cnt + 1;
            println!("iter={} entry_type={:#02x}{:#02x}", cnt, entry_type[0], entry_type[1]);
        }

        if entry_type[1] == 0xCE && (entry_type[0] == 0x07 || entry_type[0] == 0x0A) { // GlobalMark or LocalMark
            read_fully(&mut reader, &mut timestamp).expect("timestamp");
            (total_length, _) = read_uint(&mut reader);
            if DEBUG {
                println!("0x0ACE length={}", total_length);
            }

            /*
               -----------------------------------------------------
               Data contained in the map:
               Key  Type      Default  Description  
               l    UInteger  1        Position line number.  Must be
                                       greater then zero.
               c    UInteger  0        Position column number.
               n    UInteger  34 ('"') Mark name.  Only valid for
                                       GlobalMark and LocalMark
                                       entries.
               f    Binary    N/A      File name.  Required.
               *    any       none     Other keys are allowed for
                                       compatibility reasons, see
                                       |shada-compatibility|.
               -----------------------------------------------------
            */
            read_fully(&mut reader, &mut tag).expect("tag");
            processed = tag.len();

            let mut field_l = 1;
            let mut field_n = 34; // "
            let mut field_f = vec![0_0u8; 0];

            while processed < total_length {
                read_fully(&mut reader, &mut tag).expect("tag");
                processed = processed + tag.len();

                read_fully(&mut reader, &mut key).expect("key");
                processed = processed + key.len();

                match key[0] as char {
                    'l' => {
                        (length, consumed) = read_uint(&mut reader);
                        processed = processed + consumed;
                        field_l = length;
                    },
                    'c' => {
                        (_, consumed) = read_uint(&mut reader);
                        processed = processed + consumed;
                    },
                    'n' => {
                        (length, consumed) = read_uint(&mut reader);
                        processed = processed + consumed;
                        field_n = length;
                    },
                    'f' => {
                        read_fully(&mut reader, &mut tag).expect("f.tag");
                        processed = processed + tag.len();

                        (length, consumed) = read_uint(&mut reader);
                        processed = processed + consumed;

                        let mut filename = vec![0_0u8; length];
                        read_fully(&mut reader, &mut filename).expect("filename");
                        processed = processed + length;

                        field_f = filename.clone();
                    },
                    _ => {
                        panic!("unexpected key {}", key[0]);
                    },
                }
            }

            // Use [A-Za-z]
            if (field_n >= 0x41 && field_n <= 0x5A) || (field_n >= 0x61 && field_n <= 0x7A) {
                if field_f.len() > 0 && field_f[0] == b'/' {
                    match str::from_utf8(&field_f) {
                        Ok(v) => println!("{}\t{}\t{}", char::from_u32(field_n as u32).unwrap(), field_l, v),
                        Err(_) => panic!("utf8 convert fail"),
                    }
                }
            }
/*
        } else if entry_type[1] == 0xCE && entry_type[0] == 0x02 { // SearchPattern
            // I found this case from actual shada data. I need to jump 6 more bytes.
            read_fully(&mut reader, &mut timestamp).expect("timestamp");
            (total_length, _) = read_uint(&mut reader);
            if DEBUG {
                println!("0x02CE length={}", total_length);
            }
            reader.seek_relative((total_length + 6) as i64).expect("seek");
*/
        } else if entry_type[1] == 0x00 {
            if entry_type[0] > 11 {
                panic!("unexpected type: entry_type={:x?}", entry_type);
            }
            // I found this case (0x0A,0x00) from actual shada data. There's no timestamp.
            // I presume that non-0xCE value (like 0x00) means timestamp skip
            (total_length, _) = read_uint(&mut reader);
            if DEBUG {
                println!("0x??00 length={}", total_length);
            }
            reader.seek_relative(total_length as i64).expect("seek");
        } else {
            read_fully(&mut reader, &mut timestamp).expect("timestamp");
            if entry_type[0] > 11 {
                panic!("unexpected type: entry_type={:x?} timestamp={:x?}", entry_type, timestamp);
            }
            (total_length, _) = read_uint(&mut reader);
            if DEBUG {
                println!("0x???? length={}", total_length);
            }
            reader.seek_relative(total_length as i64).expect("seek");
        }
    }
}
