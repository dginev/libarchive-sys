#![feature(libc)]
//for debug purpose
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![feature(trace_macros)]
#![feature(rustc_private)]
#![feature(macro_metavar_expr_concat)]

mod ffi;
use ffi::archive::*;

use std::any::Any;
use std::ffi::CStr;
use std::ffi::CString;
use std::io::{Read, Seek};
use std::ptr;
use std::rc::Rc;

extern crate time;
use time::Duration;

#[derive(PartialEq, Clone)]
pub struct Reader {
    handler: Rc<*mut Struct_archive>,
}

#[derive(Debug)]
pub struct AllocationError;
#[derive(Debug)]
pub enum ArchiveError {
    Ok,
    Warn,
    Failed,
    Retry,
    Eof,
    Fatal,
}
#[derive(Debug)]
pub enum ArchiveExtractFlag {
    Owner,
    Perm,
    Time,
    No_Overwrite,
    Unlink,
    Acl,
    Fflags,
    Xattr,
    Secure_Symlinks,
    Secure_Nodotdot,
    No_Autodir,
    No_Overwrite_Newer,
    Sparse,
    Mac_Metadata,
    No_Hfs_Compression,
    Hfs_Compression_Forced,
    Secure_Noabsolutepaths,
}

pub enum ArchiveFormat {
    _7Zip,
    Ar_Bsd,
    Ar_Svr4,
    Cpio,
    Cpio_newc,
    Gnutar,
    Iso9600,
    Mtree,
    // Mtree_Classic,
    Pax,
    Pax_Restricted,
    Shar,
    Shar_Dump,
    Ustar,
    // V7tar,
    Xar,
    Zip,
}

pub enum ArchiveFilter {
    Bzip2,
    Compress,
    Gzip,
    Lzip,
    Lzma,
    None,
    // TODO : Program(&str)
    Xz,
}
pub enum ArchiveEntryIOType {
    ReaderEntry,
    WriterEntry,
}

pub enum ArchiveEntryFiletype {
    AE_IFMT,
    AE_IFREG,
    AE_IFLNK,
    AE_IFSOCK,
    AE_IFCHR,
    AE_IFBLK,
    AE_IFDIR,
    AE_IFIFO,
}
/*
impl fmt::Debug for AllocationError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("AllocationError").finish()
    }
}

impl fmt::Debug for AllocationError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt.debug_struct("AllocationError").finish()
    }
}*/

fn code_to_error(code: c_int) -> ArchiveError {
    match code {
        ARCHIVE_OK => ArchiveError::Ok,
        ARCHIVE_WARN => ArchiveError::Warn,
        ARCHIVE_FAILED => ArchiveError::Failed,
        ARCHIVE_RETRY => ArchiveError::Retry,
        ARCHIVE_EOF => ArchiveError::Eof,
        ARCHIVE_FATAL => ArchiveError::Fatal,
        _ => {
            panic!();
        }
    }
}

fn flags_to_code(flags: Vec<ArchiveExtractFlag>) -> c_int {
    let mut flags_code: c_int = 0;
    for flag in flags.into_iter() {
        let flag_code: c_int = match flag {
            ArchiveExtractFlag::Owner => ARCHIVE_EXTRACT_OWNER,
            ArchiveExtractFlag::Perm => ARCHIVE_EXTRACT_PERM,
            ArchiveExtractFlag::Time => ARCHIVE_EXTRACT_TIME,
            ArchiveExtractFlag::No_Overwrite => ARCHIVE_EXTRACT_NO_OVERWRITE,
            ArchiveExtractFlag::Unlink => ARCHIVE_EXTRACT_UNLINK,
            ArchiveExtractFlag::Acl => ARCHIVE_EXTRACT_ACL,
            ArchiveExtractFlag::Fflags => ARCHIVE_EXTRACT_FFLAGS,
            ArchiveExtractFlag::Xattr => ARCHIVE_EXTRACT_XATTR,
            ArchiveExtractFlag::Secure_Symlinks => ARCHIVE_EXTRACT_SECURE_SYMLINKS,
            ArchiveExtractFlag::Secure_Nodotdot => ARCHIVE_EXTRACT_SECURE_NODOTDOT,
            ArchiveExtractFlag::No_Autodir => ARCHIVE_EXTRACT_NO_AUTODIR,
            ArchiveExtractFlag::No_Overwrite_Newer => ARCHIVE_EXTRACT_NO_OVERWRITE_NEWER,
            ArchiveExtractFlag::Sparse => ARCHIVE_EXTRACT_SPARSE,
            ArchiveExtractFlag::Mac_Metadata => ARCHIVE_EXTRACT_MAC_METADATA,
            ArchiveExtractFlag::No_Hfs_Compression => ARCHIVE_EXTRACT_NO_HFS_COMPRESSION,
            ArchiveExtractFlag::Hfs_Compression_Forced => ARCHIVE_EXTRACT_HFS_COMPRESSION_FORCED,
            ArchiveExtractFlag::Secure_Noabsolutepaths => ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS,
        };
        flags_code |= flag_code;
    }
    flags_code
}

struct ReadContainer {
    reader: Box<dyn Read>,
    buffer: Vec<u8>,
    seeker: Option<Box<dyn Seek>>,
}

impl ReadContainer {
    fn read_bytes(&mut self) -> std::io::Result<usize> {
        self.reader.read(&mut self.buffer[..])
    }
}

extern "C" fn arch_read(
    arch: *mut Struct_archive,
    _client_data: *mut c_void,
    _buffer: *mut *mut c_void,
) -> ssize_t {
    unsafe {
        // use client_data as pointer to ReadContainer struct
        let mut rc = Box::from_raw(_client_data as *mut ReadContainer);
        *_buffer = rc.buffer.as_mut_ptr() as *mut c_void;
        let size = rc.read_bytes();
        let _ = Box::into_raw(rc);

        if let Err(err) = size {
            let descr = CString::new(err.to_string()).unwrap();
            archive_set_error(arch, err.raw_os_error().unwrap_or(0), descr.as_ptr());
            -1
        } else {
            size.unwrap() as ssize_t
        }
    }
}

#[allow(unused_variables)]
extern "C" fn arch_close(arch: *mut Struct_archive, _client_data: *mut c_void) -> c_int {
    unsafe {
        let rc = Box::from_raw(_client_data as *mut ReadContainer);
        ARCHIVE_OK
    }
}

extern "C" fn arch_skip(_: *mut Struct_archive, _client_data: *mut c_void, request: i64) -> i64 {
    unsafe {
        let mut rc = Box::from_raw(_client_data as *mut ReadContainer);

        // we can't return error code here, but if we return 0 normal read will be called, where error code will be set
        if rc.seeker.is_none() {
            let _ = Box::into_raw(rc);
            return 0;
        }
        let size = rc
            .seeker
            .as_mut()
            .unwrap()
            .seek(std::io::SeekFrom::Current(request))
            .unwrap_or(0);

        let _ = Box::into_raw(rc);
        size as i64
    }
}

impl Reader {
    pub fn new() -> Result<Reader, AllocationError> {
        unsafe {
            let h = archive_read_new();

            if h.is_null() {
                Err(AllocationError)
            } else {
                Ok(Reader {
                    handler: Rc::new(h),
                })
            }
        }
    }

    pub fn support_filter_all(self) -> Self {
        unsafe {
            archive_read_support_filter_all(*self.handler);
        }
        self
    }

    pub fn support_format_all(self) -> Self {
        unsafe {
            archive_read_support_format_all(*self.handler);
        }
        self
    }
    pub fn support_format_raw(self) -> Self {
        unsafe {
            archive_read_support_format_raw(*self.handler);
        }
        self
    }

    pub fn open_filename(self, fileName: &str, bufferSize: usize) -> Result<Self, ArchiveError> {
        let fname = CString::new(fileName).unwrap();
        unsafe {
            let res = archive_read_open_filename(*self.handler, fname.as_ptr(), bufferSize);
            if res == ARCHIVE_OK {
                Ok(self)
            } else {
                Err(code_to_error(res))
            }
        }
    }

    pub fn open_memory(self, memory: &mut [u8]) -> Result<Self, ArchiveError> {
        unsafe {
            let memptr: *mut u8 = &mut memory[0];
            let res = archive_read_open_memory(*self.handler, memptr as *mut c_void, memory.len());
            if res == ARCHIVE_OK {
                Ok(self)
            } else {
                Err(code_to_error(res))
            }
        }
    }

    pub fn open_stream<T: Any + Read>(self, source: T) -> Result<Self, ArchiveError> {
        unsafe {
            let mut rc_unboxed = ReadContainer {
                reader: Box::new(source),
                buffer: Vec::with_capacity(8192),
                seeker: None,
            };
            for _ in 0..8192 {
                rc_unboxed.buffer.push(0);
            }
            let rc = Box::new(rc_unboxed);

            let res = archive_read_open(
                *self.handler,
                Box::into_raw(rc) as *mut c_void,
                ptr::null_mut(),
                arch_read,
                arch_close,
            );
            if res == ARCHIVE_OK {
                Ok(self)
            } else {
                Err(code_to_error(res))
            }
        }
    }

    pub fn next_header(&self) -> Result<ArchiveEntryReader, ArchiveError> {
        use ArchiveEntryIOType::*;
        unsafe {
            let mut entry: *mut Struct_archive_entry = ptr::null_mut();
            let res = archive_read_next_header(*self.handler, &mut entry);
            if res == ARCHIVE_OK {
                Ok(ArchiveEntryReader {
                    entry,
                    handler: self.handler.clone(),
                    iotype: ReaderEntry,
                })
            } else {
                Err(code_to_error(res))
            }
        }
    }

    pub fn read_data(&self, size: size_t) -> Result<Vec<u8>, ArchiveError> {
        unsafe {
            let mut chunk_vec = Vec::with_capacity(size);
            let chunk_ptr = chunk_vec.as_mut_ptr();
            let res = archive_read_data(*self.handler, chunk_ptr as *mut c_void, size) as i32;
            if (res == ARCHIVE_FATAL) || (res == ARCHIVE_WARN) || (res == ARCHIVE_RETRY) {
                Err(code_to_error(res))
            } else if res == 0 {
                Err(code_to_error(ARCHIVE_EOF))
            } else {
                chunk_vec.set_len(res as usize);
                Ok(chunk_vec)
            }
        }
    }
}

impl Drop for ArchiveEntryReader {
    fn drop(&mut self) {
        use ArchiveEntryIOType::*;
        if Rc::strong_count(&self.handler) <= 1 {
            match self.iotype {
                ReaderEntry => unsafe {
                    archive_read_close(*self.handler);
                    archive_read_free(*self.handler);
                },
                WriterEntry => unsafe {
                    archive_write_close(*self.handler);
                    archive_write_free(*self.handler);
                },
            }
        }
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        if Rc::strong_count(&self.handler) <= 1 {
            unsafe {
                archive_read_close(*self.handler);
                archive_read_free(*self.handler);
            }
        }
    }
}

#[derive(PartialEq, Clone)]
pub struct Writer {
    handler: Rc<*mut Struct_archive>,
    outUsed: Rc<*mut size_t>,
}

impl Drop for Writer {
    fn drop(&mut self) {
        if Rc::strong_count(&self.handler) <= 1 {
            unsafe {
                archive_write_close(*self.handler);
                archive_write_free(*self.handler);
            }
        }
    }
}

impl Writer {
    pub fn new() -> Result<Writer, AllocationError> {
        unsafe {
            let h = archive_write_new();
            if h.is_null() {
                Err(AllocationError)
            } else {
                let mut init_used: Box<size_t> = Box::new(0);
                let outUsed: *mut size_t = &mut *init_used;
                Ok(Writer {
                    handler: Rc::new(h),
                    outUsed: Rc::new(outUsed),
                })
            }
        }
    }
    pub fn add_filter(self, filter: ArchiveFilter) -> Self {
        unsafe {
            match filter {
                ArchiveFilter::Bzip2 => archive_write_add_filter_bzip2(*self.handler),
                ArchiveFilter::Compress => archive_write_add_filter_compress(*self.handler),
                ArchiveFilter::Gzip => archive_write_add_filter_gzip(*self.handler),
                ArchiveFilter::Lzip => archive_write_add_filter_lzip(*self.handler),
                ArchiveFilter::Lzma => archive_write_add_filter_lzma(*self.handler),
                ArchiveFilter::None => archive_write_add_filter_none(*self.handler),
                // TODO : Program(&str)
                ArchiveFilter::Xz => archive_write_add_filter_xz(*self.handler),
            };
        }
        self
    }

    pub fn set_format(self, format: ArchiveFormat) -> Self {
        unsafe {
            match format {
                ArchiveFormat::_7Zip => archive_write_set_format_7zip(*self.handler),
                ArchiveFormat::Ar_Bsd => archive_write_set_format_ar_bsd(*self.handler),
                ArchiveFormat::Ar_Svr4 => archive_write_set_format_ar_svr4(*self.handler),
                ArchiveFormat::Cpio => archive_write_set_format_cpio(*self.handler),
                ArchiveFormat::Cpio_newc => archive_write_set_format_cpio_newc(*self.handler),
                ArchiveFormat::Gnutar => archive_write_set_format_gnutar(*self.handler),
                ArchiveFormat::Iso9600 => archive_write_set_format_iso9660(*self.handler),
                ArchiveFormat::Mtree => archive_write_set_format_mtree(*self.handler),
                // ArchiveFormat::Mtree_Classic => archive_write_set_format_mtree_classic(*self.handler),
                ArchiveFormat::Pax => archive_write_set_format_pax(*self.handler),
                ArchiveFormat::Pax_Restricted => {
                    archive_write_set_format_pax_restricted(*self.handler)
                }
                ArchiveFormat::Shar => archive_write_set_format_shar(*self.handler),
                ArchiveFormat::Shar_Dump => archive_write_set_format_shar_dump(*self.handler),
                ArchiveFormat::Ustar => archive_write_set_format_ustar(*self.handler),
                // ArchiveFormat::V7tar => archive_write_set_format_v7tar(*self.handler),
                ArchiveFormat::Xar => archive_write_set_format_xar(*self.handler),
                ArchiveFormat::Zip => archive_write_set_format_zip(*self.handler),
            };
        }
        self
    }

    pub fn set_compression(self, filter: ArchiveFilter) -> Self {
        unsafe {
            match filter {
                ArchiveFilter::Bzip2 => archive_write_set_compression_bzip2(*self.handler),
                ArchiveFilter::Compress => archive_write_set_compression_compress(*self.handler),
                ArchiveFilter::Gzip => archive_write_set_compression_gzip(*self.handler),
                ArchiveFilter::Lzip => archive_write_set_compression_lzip(*self.handler),
                ArchiveFilter::Lzma => archive_write_set_compression_lzma(*self.handler),
                ArchiveFilter::None => archive_write_set_compression_none(*self.handler),
                ArchiveFilter::Xz => archive_write_set_compression_xz(*self.handler),
            };
        }
        self
    }

    pub fn open_filename(&mut self, fileName: &str) -> Result<&mut Self, ArchiveError> {
        let fname = CString::new(fileName).unwrap();
        unsafe {
            let res = archive_write_open_filename(*self.handler, fname.as_ptr());
            if res == ARCHIVE_OK {
                Ok(self)
            } else {
                Err(code_to_error(res))
            }
        }
    }

    pub fn open_memory(&mut self, memory: &mut [u8]) -> Result<&mut Self, ArchiveError> {
        unsafe {
            let memptr: *mut u8 = &mut memory[0];
            let res = archive_write_open_memory(
                *self.handler,
                memptr as *mut c_void,
                memory.len(),
                *self.outUsed,
            );
            if res == ARCHIVE_OK {
                Ok(self)
            } else {
                Err(code_to_error(res))
            }
        }
    }

    pub fn write_header(&mut self, entry: ArchiveEntryReader) -> Result<&mut Self, ArchiveError> {
        unsafe {
            let res = archive_write_header(*self.handler, entry.entry);
            if res == ARCHIVE_OK {
                Ok(self)
            } else {
                Err(code_to_error(res))
            }
        }
    }

    pub fn write_header_new(
        &mut self,
        pathname: &str,
        entry_size: i64,
    ) -> Result<&mut Self, ArchiveError> {
        use ArchiveEntryIOType::*;
        unsafe {
            let new_entry = archive_entry_new();
            archive_entry_set_perm(new_entry, 0o755);
            archive_entry_set_size(new_entry, entry_size);
            let entry = ArchiveEntryReader {
                entry: new_entry,
                handler: self.handler.clone(),
                iotype: WriterEntry,
            };
            entry.set_filetype(ArchiveEntryFiletype::AE_IFREG);
            entry.set_pathname(pathname);

            self.write_header(entry)
        }
    }

    pub fn write_data(&mut self, data: Vec<u8>) -> Result<&mut Self, ArchiveError> {
        unsafe {
            let data_len = data.len();
            let data_bytes = CString::from_vec_unchecked(data);
            // TODO: How to handle errors here?
            archive_write_data(*self.handler, data_bytes.as_ptr() as *mut c_void, data_len);
        }
        Ok(self)
    }
    pub fn write_finish_entry(&mut self) -> Result<&mut Self, ArchiveError> {
        unsafe {
            let res = archive_write_finish_entry(*self.handler);
            if res == ARCHIVE_OK {
                Ok(self)
            } else {
                Err(code_to_error(res))
            }
        }
    }
}

#[derive(PartialEq, Clone)]
pub struct WriterToDisk {
    handler: Rc<*mut Struct_archive>,
}

impl WriterToDisk {
    pub fn new() -> Result<WriterToDisk, AllocationError> {
        unsafe {
            let h = archive_write_disk_new();
            if h.is_null() {
                Err(AllocationError)
            } else {
                Ok(WriterToDisk {
                    handler: Rc::new(h),
                })
            }
        }
    }
}

impl Drop for WriterToDisk {
    fn drop(&mut self) {
        if Rc::strong_count(&self.handler) <= 1 {
            unsafe {
                archive_write_close(*self.handler);
                archive_write_free(*self.handler);
            }
        }
    }
}

pub struct ArchiveEntryReader {
    entry: *mut Struct_archive_entry,
    handler: Rc<*mut Struct_archive>,
    iotype: ArchiveEntryIOType,
}

macro_rules! get_time {
    ( $fname:ident, $apiname:ident) => {
        pub fn $fname(&self) -> Duration {
            unsafe {
                let sec = (${concat(archive_entry_, $apiname)})(self.entry);
                let nsec = (${concat(archive_entry_, $apiname, _nsec)})(self.entry);
                Duration::new(sec, nsec as i32)
            }
        }
    };
}

unsafe fn wrap_to_string(ptr: *const c_char) -> String {
    let path = CStr::from_ptr(ptr);
    String::from(std::str::from_utf8(path.to_bytes()).unwrap())
}

impl ArchiveEntryReader {
    pub fn size(&self) -> i64 {
        unsafe { archive_entry_size(self.entry) }
    }

    pub fn pathname(&self) -> String {
        unsafe { wrap_to_string(archive_entry_pathname(self.entry)) }
    }

    pub fn sourcepath(&self) -> String {
        unsafe { wrap_to_string(archive_entry_sourcepath(self.entry)) }
    }

    pub fn set_filetype(&self, filetype: ArchiveEntryFiletype) {
        let c_type = match filetype {
            ArchiveEntryFiletype::AE_IFMT => 0o170_000,
            ArchiveEntryFiletype::AE_IFREG => 0o100_000,
            ArchiveEntryFiletype::AE_IFLNK => 0o120_000,
            ArchiveEntryFiletype::AE_IFSOCK => 0o140_000,
            ArchiveEntryFiletype::AE_IFCHR => 0o020_000,
            ArchiveEntryFiletype::AE_IFBLK => 0o060_000,
            ArchiveEntryFiletype::AE_IFDIR => 0o040_000,
            ArchiveEntryFiletype::AE_IFIFO => 0o010_000,
        };
        unsafe {
            archive_entry_set_filetype(self.entry, c_type);
        }
    }

    pub fn set_pathname(&self, pathname: &str) {
        let c_pathname = CString::new(pathname).unwrap();
        unsafe {
            archive_entry_set_pathname(self.entry, c_pathname.as_ptr());
        }
    }

    pub fn archive(&self) -> Reader {
        Reader {
            handler: self.handler.clone(),
        }
    }

    pub fn extract_to(
        self,
        path: &str,
        flags: Vec<ArchiveExtractFlag>,
    ) -> Result<Self, ArchiveError> {
        let extract_path = CString::new(path).unwrap();
        unsafe {
            archive_entry_set_pathname(self.entry, extract_path.as_ptr());
            self.extract(flags)
        }
    }
    pub fn extract(self, flags: Vec<ArchiveExtractFlag>) -> Result<Self, ArchiveError> {
        unsafe {
            let res = archive_read_extract(*self.handler, self.entry, flags_to_code(flags));
            if res == ARCHIVE_OK {
                Ok(self)
            } else {
                Err(code_to_error(res))
            }
        }
    }

    get_time!(access_time, atime);
    get_time!(creation_time, birthtime);
    get_time!(inode_change_time, ctime);
    get_time!(modification_time, mtime);
}
