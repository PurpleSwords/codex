//! Bounded, best-effort output capture for the WSL terminal environment probe.

use std::io;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

const MAX_OUTPUT_BYTES: usize = 8 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(super) fn read_output(command: &mut Command, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
    let start = Instant::now();
    let mut child = command.stdout(Stdio::piped()).spawn()?;
    let result = (|| {
        let mut stdout = child.stdout.take().expect("probe stdout is piped");
        let fd = stdout.as_raw_fd();
        // SAFETY: stdout owns this live descriptor throughout both fcntl calls.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags == -1 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: F_SETFL only changes status flags on the owned pipe descriptor.
        if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
            return Err(io::Error::last_os_error());
        }

        let mut output = Vec::new();
        let mut buffer = [0; 1024];
        loop {
            // Observe exit before draining so the final bytes cannot be missed.
            let status = child.try_wait()?;
            loop {
                if start.elapsed() >= timeout {
                    return Ok(None);
                }
                match stdout.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(len) => {
                        if output.len() + len > MAX_OUTPUT_BYTES {
                            return Ok(None);
                        }
                        output.extend_from_slice(&buffer[..len]);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error),
                }
            }
            if let Some(status) = status {
                // A descendant may retain stdout: never wait for pipe EOF.
                return Ok(status.success().then_some(output));
            }
            std::thread::sleep(POLL_INTERVAL.min(timeout.saturating_sub(start.elapsed())));
        }
    })();

    // Also clean up on read errors or excess output. Successful exits have
    // already been reaped by try_wait; kill then has no effect.
    let _ = child.kill();
    let _ = child.wait();
    result
}

#[cfg(test)]
#[path = "windows_terminal_probe_tests.rs"]
mod tests;
