//! Probe: what screens does `claude` show WITHOUT --dangerously-skip-permissions
//! when spawned programmatically in a fresh git worktree, and how does a
//! tool-permission request surface? Informs the product's session-launch +
//! approval flow (architecture §4.3).

use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn sh(dir: &Path, args: &[&str]) -> Result<()> {
    let st = Command::new(args[0]).args(&args[1..]).current_dir(dir).status()?;
    assert!(st.success(), "cmd failed: {:?}", args);
    Ok(())
}

fn dump(label: &str, buf: &Arc<Mutex<Vec<u8>>>) {
    let raw = buf.lock().unwrap().clone();
    // strip most ANSI to read the text content
    let s = String::from_utf8_lossy(&raw);
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // skip CSI / OSC until a letter or BEL
            while let Some(&n) = chars.peek() {
                chars.next();
                if n.is_ascii_alphabetic() || n == '\u{7}' {
                    break;
                }
            }
        } else if c == '\r' {
            // collapse
        } else {
            out.push(c);
        }
    }
    println!("==== {} ====", label);
    for line in out.lines() {
        let t = line.trim();
        if !t.is_empty() {
            println!("{}", t);
        }
    }
    println!("==== end {} ====\n", label);
}

fn main() -> Result<()> {
    let root = PathBuf::from("/private/tmp/weft-probe");
    let _ = std::fs::remove_dir_all(&root);
    let repo = root.join("demo-repo");
    let wt = root.join("wt");
    std::fs::create_dir_all(&repo)?;
    sh(&repo, &["git", "init", "-q"])?;
    sh(&repo, &["git", "config", "user.email", "t@t.t"])?;
    sh(&repo, &["git", "config", "user.name", "t"])?;
    std::fs::write(repo.join("README.md"), "# demo\n")?;
    sh(&repo, &["git", "add", "-A"])?;
    sh(&repo, &["git", "commit", "-q", "-m", "init"])?;
    sh(&repo, &["git", "worktree", "add", "-q", "-b", "ws/demo/t1/main", wt.to_str().unwrap()])?;

    let pair = native_pty_system().openpty(PtySize { rows: 45, cols: 130, pixel_width: 0, pixel_height: 0 })?;
    let mut cmd = CommandBuilder::new("claude"); // NO --dangerously-skip-permissions
    cmd.cwd(&wt);
    for (k, v) in std::env::vars() {
        cmd.env(k, v);
    }
    let mut child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;
    let buf = Arc::new(Mutex::new(Vec::new()));
    let b2 = buf.clone();
    std::thread::spawn(move || {
        let mut tmp = [0u8; 4096];
        loop {
            match reader.read(&mut tmp) {
                Ok(0) | Err(_) => break,
                Ok(n) => b2.lock().unwrap().extend_from_slice(&tmp[..n]),
            }
        }
    });

    std::thread::sleep(Duration::from_secs(5));
    dump("SCREEN 1 (just booted -> trust dialog)", &buf);

    // GATE FIRST: answer the trust dialog before sending any prompt.
    buf.lock().unwrap().clear();
    write!(writer, "1\r")?;
    writer.flush()?;
    std::thread::sleep(Duration::from_secs(3));

    // now the real prompt
    buf.lock().unwrap().clear();
    write!(writer, "Create a file named hi.txt containing exactly: hi\r")?;
    writer.flush()?;
    std::thread::sleep(Duration::from_secs(12));
    dump("SCREEN 2 (tool permission request)", &buf);

    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}
