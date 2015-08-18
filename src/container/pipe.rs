use std::io::Error as IoError;
use std::io::{Read, Write};
use std::fs::File;
use std::os::unix::io::{RawFd, FromRawFd};
use nix::unistd::{pipe};
use nix::Error::{Sys, InvalidPath};

use libc::funcs::posix88::unistd::{close};


pub struct CPipe {
    pub reader: RawFd,
    pub writer: RawFd,
}

impl CPipe {
    pub fn new() -> Result<CPipe, IoError> {
        match pipe() {
            Ok((reader, writer)) => Ok(CPipe {
                reader: reader, writer: writer
            }),
            Err(Sys(code)) => Err(IoError::from_raw_os_error(code as i32)),
            Err(InvalidPath) => unreachable!(),
        }
    }
    pub fn read(self) -> Result<Vec<u8>, IoError> {
        let mut buf = Vec::new();
        unsafe {
          close(self.writer);
          try!(File::from_raw_fd(self.reader).read_to_end(&mut buf));
        }
        Ok(buf)
    }
    pub fn wakeup(self) -> Result<(), IoError> {
        unsafe {
          close(self.reader);
          File::from_raw_fd(self.writer).write_all(b"x")
        }
    }
}

impl Drop for CPipe {
    fn drop(&mut self) {
        unsafe {
            close(self.reader);
            close(self.writer);
        }
    }
}
