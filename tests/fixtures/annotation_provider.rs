use std::env;
use std::io::{self, Read, Write};
use std::thread;
use std::time::Duration;

fn write_json_string(mut output: impl Write, bytes: &[u8]) -> io::Result<()> {
    output.write_all(b"\"")?;
    for &byte in bytes {
        match byte {
            b'"' => output.write_all(br#"\""#)?,
            b'\\' => output.write_all(br#"\\"#)?,
            b'\n' => output.write_all(br#"\n"#)?,
            b'\r' => output.write_all(br#"\r"#)?,
            b'\t' => output.write_all(br#"\t"#)?,
            0x08 => output.write_all(br#"\b"#)?,
            0x0c => output.write_all(br#"\f"#)?,
            0x00..=0x1f => write!(output, "\\u{byte:04x}")?,
            _ => output.write_all(&[byte])?,
        }
    }
    output.write_all(b"\"")
}

fn write_event(text: &[u8]) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(b"{\"time\":\"2026-08-12T10:02:00Z\",\"text\":")?;
    write_json_string(&mut stdout, text)?;
    stdout.write_all(b"}\n")
}

fn write_repeated(mut output: impl Write, count: usize, byte: u8) -> io::Result<()> {
    let chunk = [byte; 8192];
    let mut remaining = count;
    while remaining > 0 {
        let amount = remaining.min(chunk.len());
        output.write_all(&chunk[..amount])?;
        remaining -= amount;
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut request = Vec::new();
    io::stdin().read_to_end(&mut request)?;

    match args.first().map(String::as_str) {
        Some("echo-request") => write_event(&request)?,
        Some("show-context") => {
            let inherited_key = args.last().expect("environment key");
            let text = format!(
                "args={} cwd={} env={}",
                args[1..].join("|"),
                env::current_dir()?.display(),
                env::var(inherited_key).unwrap_or_default(),
            );
            write_event(text.as_bytes())?;
        }
        Some("empty") => {}
        Some("invalid") if args.get(1).map(String::as_str) == Some("utf8") => {
            io::stdout().write_all(&[0xff])?;
        }
        Some("invalid") => io::stdout().write_all(b"{invalid}\n")?,
        Some("exit") => {
            io::stdout().write_all(b"{invalid}\n")?;
            eprintln!("first diagnostic line");
            eprintln!("second diagnostic line");
            std::process::exit(args[1].parse().expect("exit code"));
        }
        Some("sleep") => {
            thread::sleep(Duration::from_millis(
                args[1].parse().expect("sleep milliseconds"),
            ));
            write_event(b"awake")?;
        }
        Some("stdout-bytes") => write_repeated(
            io::stdout().lock(),
            args[1].parse().expect("stdout byte count"),
            b'x',
        )?,
        Some("stderr-bytes") => {
            write_repeated(
                io::stderr().lock(),
                args[1].parse().expect("stderr byte count"),
                b'e',
            )?;
            std::process::exit(args[2].parse().expect("exit code"));
        }
        _ => panic!("unknown fixture mode"),
    }

    Ok(())
}
