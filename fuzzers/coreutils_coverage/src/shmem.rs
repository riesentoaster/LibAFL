use std::{io::Error as IOError, path::Path, process::Command};

use libafl::Error;
use libafl_bolts::shmem::ShMemDescription;
use libc::{fcntl, FD_CLOEXEC, F_GETFD, F_SETFD};

pub fn get_guard_num(util: &str) -> Result<usize, Error> {
    if !Path::new(util).exists() {
        panic!("Missing util binary.\nCompile it using `cargo make bin` first.",);
    }
    let shared = "./target/release/libget_guard_num.so";
    if !Path::new(shared).exists() {
        panic!("Missing shared library to instrument binary to find number of edges.\nCompile it using `cargo make guard_num` first.");
    }

    let guard_num_command_output = Command::new(util)
        .env("LD_PRELOAD", shared)
        .output()?
        .stdout;
    let guard_num = String::from_utf8(guard_num_command_output)?
        .trim()
        .parse::<usize>()?;

    println!("Got guard_num {}", guard_num);
    match guard_num {
        0 => Err(Error::illegal_state("Binary reported a guard count of 0")),
        e => Ok(e),
    }
}

pub fn make_shmem_persist(description: &ShMemDescription) {
    let fd = description.id.as_str().parse().unwrap();
    let flags = unsafe { fcntl(fd, F_GETFD) };

    if flags == -1 {
        panic!("Failed to get FD flags: {}", IOError::last_os_error());
    }
    let result = unsafe { fcntl(fd, F_SETFD, flags & !FD_CLOEXEC) };
    if result == -1 {
        panic!("Failed to set FD flags: {}", IOError::last_os_error());
    }
}
