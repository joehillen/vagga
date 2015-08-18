use std::io::{BufReader, BufRead, Read};
use std::fs::File;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use rustc_serialize::json;

use config::Config;
use config::read_config;
use config::builders::{Builder};
use config::builders::Builder as B;
use config::builders::Source as S;
use shaman::digest::Digest;
use container::vagga::container_ver;
use self::HashResult::*;


pub enum HashResult {
    Hashed,
    New,
    Error(String)
}


pub trait VersionHash {
    fn hash(&self, cfg: &Config, hash: &mut Digest) -> HashResult;
}


impl VersionHash for Builder {
    fn hash(&self, cfg: &Config, hash: &mut Digest) -> HashResult {
        match self {
            &B::Py2Requirements(ref fname) | &B::Py3Requirements(ref fname)
            => {
                match
                    File::open(&Path::new("/work").join(fname))
                    .and_then(|f| {
                        let f = BufReader::new(f);
                        for line in f.lines() {
                            let line = try!(line);
                            let chunk = line[..].trim();
                            // Ignore empty lines and comments
                            if chunk.len() == 0 || chunk.starts_with("#") {
                                continue;
                            }
                            // Should we also ignore the order?
                            hash.input(chunk.as_bytes());
                        }
                        Ok(())
                    })
                {
                    Err(e) => return Error(format!("Can't read file: {}", e)),
                    Ok(()) => return Hashed,
                }
            }
            &B::Depends(ref filename) => {
                match
                    File::open(&Path::new("/work").join(filename))
                    .and_then(|mut f| {
                        loop {
                            let mut chunk = [0u8; 128*1024];
                            let bytes = match f.read(&mut chunk[..]) {
                                Ok(0) => break,
                                Ok(bytes) => bytes,
                                Err(e) => return Err(e),
                            };
                            hash.input(&chunk[..bytes]);
                        }
                        Ok(())
                    })
                {
                    Err(e) => return Error(format!("Can't read file: {}", e)),
                    Ok(()) => return Hashed,
                }
            }
            &B::Container(ref name) => {
                let cont = match cfg.containers.get(name) {
                    Some(cont) => cont,
                    None => {
                        return Error(format!("Container {:?} not found",
                                             name));
                    }
                };
                for b in cont.setup.iter() {
                    debug!("Versioning setup: {:?}", b);
                    match b.hash(cfg, hash) {
                        Hashed => continue,
                        New => return New,  // Always rebuild
                        Error(e) => {
                            return Error(format!("{:?}: {}", name, e));
                        }
                    }
                }
                Hashed
            }
            &B::SubConfig(ref sconfig) => {
                let path = match sconfig.source {
                    S::Container(ref container) => {
                        let version = match container_ver(container) {
                            Ok(ver) => ver,
                            Err(_) => return New,  // TODO(tailhook) better check
                        };
                        Path::new("/vagga/base/.roots")
                            .join(version).join("root")
                            .join(&sconfig.path)
                    }
                    S::Git(ref git) => {
                        unimplemented!();
                    }
                    S::Directory => {
                        Path::new("/work").join(&sconfig.path)
                    }
                };
                let subcfg = match read_config(&path) {
                    Ok(cfg) => cfg,
                    Err(e) => return Error(e),
                };
                let cont = match subcfg.containers.get(&sconfig.container) {
                    Some(cont) => cont,
                    None => {
                        return Error(format!(
                            "Container {:?} not found in {:?}",
                            sconfig.container, sconfig.path));
                    }
                };
                for b in cont.setup.iter() {
                    debug!("Versioning setup: {:?}", b);
                    match b.hash(cfg, hash) {
                        Hashed => continue,
                        New => return New,  // Always rebuild
                        Error(e) => {
                            return Error(format!("{:?}: {}",
                                sconfig.container, e));
                        }
                    }
                }
                Hashed
            }
            &B::CacheDirs(ref map) => {
                for (k, v) in map.iter() {
                    hash.input(k.as_os_str().as_bytes());
                    hash.input(b"\0");
                    hash.input(v.as_bytes());
                    hash.input(b"\0");
                }
                Hashed
            }
            &B::Text(ref map) => {
                for (k, v) in map.iter() {
                    hash.input(k.as_os_str().as_bytes());
                    hash.input(b"\0");
                    hash.input(v.as_bytes());
                    hash.input(b"\0");
                }
                Hashed
            }
            _ => {
                hash.input(json::encode(self).unwrap().as_bytes());
                Hashed
            }
        }
    }
}
