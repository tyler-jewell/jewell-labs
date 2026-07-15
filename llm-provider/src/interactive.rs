//! `expect` subcommand — drive an interactive command through a PTY (answer a password
//! prompt, a sudo challenge, a y/n question) without tmux or expect(1). Pure Rust.
//!
//! The command to run follows `--`; the step-script is read from stdin, one step per line
//! (`#` comments and blank lines ignored):
//!
//!   expect <regex>   wait until the PTY output matches <regex> (bounded by --timeout)
//!   send   <text>    type <text> + Enter
//!   secret <path>    type the (trimmed) contents of a 0600 file + Enter, never logged
//!
//! Example (install this Mac's SSH key on the VPS):
//!   printf 'expect password:\nsecret ~/.vps_pw\nexpect Number of key|already exist\n' \
//!     | llm-provider expect --timeout 30 -- \
//!         ssh-copy-id -o StrictHostKeyChecking=accept-new -o PubkeyAuthentication=no \
//!                     -i ~/.ssh/id_ed25519.pub root@2.25.132.76

use std::io::{BufRead, Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use regex::bytes::Regex;

/// Runs the subcommand; returns the process exit code (child's code, or non-zero on error).
pub fn run(args: &[String]) -> i32 {
    let mut timeout = Duration::from_secs(30);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--timeout" => {
                let secs: u64 = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
                if secs == 0 {
                    eprintln!("expect: --timeout needs a positive integer (seconds)");
                    return 2;
                }
                timeout = Duration::from_secs(secs);
                i += 2;
            }
            "--" => {
                i += 1;
                break;
            }
            other => {
                eprintln!("expect: unexpected arg '{other}' (usage: expect [--timeout N] -- CMD ARGS...)");
                return 2;
            }
        }
    }
    let cmd = &args[i..];
    if cmd.is_empty() {
        eprintln!("expect: no command after '--'");
        return 2;
    }

    let pair = match native_pty_system().openpty(PtySize {
        rows: 50,
        cols: 200,
        pixel_width: 0,
        pixel_height: 0,
    }) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("expect: openpty failed: {e}");
            return 1;
        }
    };

    let mut builder = CommandBuilder::new(&cmd[0]);
    builder.args(&cmd[1..]);
    let mut child = match pair.slave.spawn_command(builder) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("expect: failed to spawn '{}': {e}", cmd[0]);
            return 1;
        }
    };
    drop(pair.slave); // otherwise the master read never sees EOF

    // Background reader: append all PTY output to a shared buffer.
    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let mut reader = pair.master.try_clone_reader().expect("pty reader");
    {
        let buf = buf.clone();
        std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.lock().unwrap().extend_from_slice(&chunk[..n]),
                }
            }
        });
    }
    let mut writer = pair.master.take_writer().expect("pty writer");

    let mut search_from = 0usize; // only match output produced after the last match
    for line in std::io::stdin().lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (verb, rest) = match line.split_once(char::is_whitespace) {
            Some((v, r)) => (v, r.trim()),
            None => (line, ""),
        };
        match verb {
            "expect" => {
                let re = match Regex::new(rest) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("expect: bad regex /{rest}/: {e}");
                        let _ = child.kill();
                        return 2;
                    }
                };
                match wait_for(&buf, &re, search_from, timeout) {
                    Some(end) => {
                        search_from = end;
                        eprintln!("[expect] matched /{rest}/");
                    }
                    None => {
                        eprintln!("[expect] TIMEOUT after {}s waiting for /{rest}/", timeout.as_secs());
                        eprintln!("--- last PTY output ---\n{}", tail_lossy(&buf, 800));
                        let _ = child.kill();
                        return 1;
                    }
                }
            }
            "send" => {
                if writeln!(writer, "{rest}").and_then(|_| writer.flush()).is_err() {
                    eprintln!("expect: write failed (child gone?)");
                    let _ = child.kill();
                    return 1;
                }
            }
            "secret" => {
                let s = match std::fs::read_to_string(rest) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("expect: cannot read secret file '{rest}': {e}");
                        let _ = child.kill();
                        return 1;
                    }
                };
                let s = s.trim_end_matches(['\n', '\r']);
                if writeln!(writer, "{s}").and_then(|_| writer.flush()).is_err() {
                    eprintln!("expect: write failed (child gone?)");
                    let _ = child.kill();
                    return 1;
                }
                eprintln!("[expect] sent secret from {rest}");
            }
            other => {
                eprintln!("expect: unknown step '{other}' (expect|send|secret)");
                let _ = child.kill();
                return 2;
            }
        }
    }

    // Wait for the command to finish, bounded so an unscripted prompt can't hang us forever.
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() { 0 } else { status.exit_code() as i32 };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    eprintln!("expect: command still running after {}s — killing", timeout.as_secs());
                    eprintln!("--- last PTY output ---\n{}", tail_lossy(&buf, 800));
                    let _ = child.kill();
                    return 1;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                eprintln!("expect: wait failed: {e}");
                return 1;
            }
        }
    }
}

/// Poll the shared buffer until `re` matches the bytes after `from`, or the timeout elapses.
/// Returns the absolute byte offset just past the match (to advance the search cursor).
fn wait_for(buf: &Arc<Mutex<Vec<u8>>>, re: &Regex, from: usize, timeout: Duration) -> Option<usize> {
    let deadline = Instant::now() + timeout;
    loop {
        {
            let b = buf.lock().unwrap();
            if from <= b.len() {
                if let Some(m) = re.find(&b[from..]) {
                    return Some(from + m.end());
                }
            }
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn tail_lossy(buf: &Arc<Mutex<Vec<u8>>>, n: usize) -> String {
    let b = buf.lock().unwrap();
    let start = b.len().saturating_sub(n);
    String::from_utf8_lossy(&b[start..]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_for_matches_advances_cursor_and_avoids_stale_rematch() {
        let buf = Arc::new(Mutex::new(b"login: alice\npassword: ".to_vec()));
        let short = Duration::from_millis(100);

        // First expect matches and returns the offset just past "password:".
        let re_pw = Regex::new("password:").unwrap();
        let cur = wait_for(&buf, &re_pw, 0, short).expect("password: should match");
        assert_eq!(cur, b"login: alice\npassword:".len());

        // Searching for "login:" from the advanced cursor must NOT rematch the earlier line.
        let re_login = Regex::new("login:").unwrap();
        assert!(wait_for(&buf, &re_login, cur, short).is_none());

        // But a fresh "login:" appended after the cursor is found.
        buf.lock().unwrap().extend_from_slice(b"\nlogin: ok");
        assert!(wait_for(&buf, &re_login, cur, short).is_some());
    }

    #[test]
    fn wait_for_times_out_when_absent() {
        let buf = Arc::new(Mutex::new(b"nothing here".to_vec()));
        let re = Regex::new("ABSENT").unwrap();
        assert!(wait_for(&buf, &re, 0, Duration::from_millis(80)).is_none());
    }
}
