//! A Java `InputStream` / `OutputStream` (a Bluetooth socket's) as Rust `Read` / `Write`. Each call goes through JNI.
#![cfg(target_os = "android")]
use std::{io, sync::Arc};

use jni::{
    objects::{GlobalRef, JValue},
    JavaVM,
};

pub struct JStream {
    pub vm: Arc<JavaVM>,
    pub obj: GlobalRef,
}

fn io_err(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e.to_string())
}

impl io::Read for JStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut env = self.vm.attach_current_thread().map_err(io_err)?;
        let arr = env.new_byte_array(buf.len() as i32).map_err(io_err)?;
        let r = env.call_method(&self.obj, "read", "([BII)I", &[JValue::Object(&arr), JValue::Int(0), JValue::Int(buf.len() as i32)]);
        let n = match r {
            Ok(v) => v.i().map_err(io_err)?,
            Err(e) => {
                let _ = env.exception_clear();
                let _ = env.delete_local_ref(arr);
                return Err(io_err(e));
            }
        };
        if n <= 0 {
            let _ = env.delete_local_ref(arr);
            return Ok(0);
        }
        let mut tmp = vec![0i8; n as usize];
        let ok = env.get_byte_array_region(&arr, 0, &mut tmp);
        let _ = env.delete_local_ref(arr);
        ok.map_err(io_err)?;
        for (d, s) in buf.iter_mut().zip(tmp.iter()) {
            *d = *s as u8;
        }
        Ok(n as usize)
    }
}

impl io::Write for JStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut env = self.vm.attach_current_thread().map_err(io_err)?;
        let arr = env.byte_array_from_slice(buf).map_err(io_err)?;
        let r = env.call_method(&self.obj, "write", "([B)V", &[JValue::Object(&arr)]);
        let _ = env.delete_local_ref(arr);
        if let Err(e) = r {
            let _ = env.exception_clear();
            return Err(io_err(e));
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut env = self.vm.attach_current_thread().map_err(io_err)?;
        if let Err(e) = env.call_method(&self.obj, "flush", "()V", &[]) {
            let _ = env.exception_clear();
            return Err(io_err(e));
        }
        Ok(())
    }
}
