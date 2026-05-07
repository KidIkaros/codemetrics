// ═══════════════════════════════════════════
// PROGRESS / SPINNER
// ═══════════════════════════════════════════

use std::time::Instant;
use colored::Colorize;

/// Run `f` on the current thread while a spinner ticks on a background thread.
/// Returns the result of `f`. The spinner shows elapsed time in real-time.
pub fn run_with_spinner<T, F>(label: &str, f: F) -> T
where
    F: FnOnce() -> T,
    T: Send + 'static,
{
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let tty = is_tty();
    let label_str = label.to_string();
    let done = Arc::new(AtomicBool::new(false));
    let done_clone = done.clone();
    let start = Instant::now();

    let ticker = std::thread::spawn(move || {
        if !tty {
            eprintln!("  … {}", label_str);
            return;
        }
        let mut frame = 0usize;
        let mut last_len = 0usize;
        loop {
            if done_clone.load(Ordering::Relaxed) {
                break;
            }
            let f = SPINNER_FRAMES[frame % SPINNER_FRAMES.len()];
            frame += 1;
            let elapsed = format_elapsed(start.elapsed());
            let line = format!("  {} {}  {}", f.cyan(), label_str, elapsed.bright_black());
            eprint!("\r{:<width$}", line, width = last_len.max(line.len()));
            last_len = line.len();
            let _ = std::io::Write::flush(&mut std::io::stderr());
            std::thread::sleep(std::time::Duration::from_millis(80));
        }
    });

    let result = f();
    done.store(true, Ordering::Relaxed);
    let _ = ticker.join();
    result
}

/// Detect whether stderr is a real TTY (not CI, not piped).
pub fn is_tty() -> bool {
    if std::env::var("CI").is_ok()
        || std::env::var("NO_COLOR").is_ok()
        || std::env::var("CODEMETRICS_NO_PROGRESS").is_ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        unsafe { libc_isatty(2) }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

#[cfg(unix)]
unsafe fn libc_isatty(fd: i32) -> bool {
    extern "C" {
        fn isatty(fd: i32) -> i32;
    }
    isatty(fd) != 0
}

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// An overall progress bar for multi-step operations (run_batch).
pub struct Bar {
    pub total: usize,
    pub done: usize,
    pub start: Instant,
    pub tty: bool,
    pub last_len: usize,
    pub current_tool: String,
}

impl Bar {
    pub fn new(total: usize) -> Self {
        let tty = is_tty();
        Self {
            total,
            done: 0,
            start: Instant::now(),
            tty,
            last_len: 0,
            current_tool: String::new(),
        }
    }

    pub fn set_current(&mut self, tool: &str) {
        self.current_tool = tool.to_string();
        self.render();
    }

    pub fn advance(&mut self, tool: &str, passed: bool, duration_ms: u64) {
        self.done += 1;
        let icon = if passed {
            "  ✓".green().bold()
        } else {
            "  ✗".red().bold()
        };
        let name_col = if passed { tool.normal() } else { tool.red() };
        let dur_str = format_ms(duration_ms);
        if self.tty {
            eprintln!("\r{:<width$}", "", width = self.last_len);
            eprintln!("\r{} {:<18}  {}", icon, name_col, dur_str.bright_black());
        } else {
            let ci_icon = if passed { "✓" } else { "✗" };
            eprintln!("  {} {:<18}  {}", ci_icon, tool, dur_str);
        }
        self.render();
    }

    pub fn render(&mut self) {
        if !self.tty {
            return;
        }
        let pct = if self.total > 0 {
            self.done * 100 / self.total
        } else {
            0
        };
        let bar_width = 28usize;
        let filled = bar_width * self.done / self.total.max(1);
        let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);
        let elapsed = self.start.elapsed();
        let eta_str = if self.done > 0 {
            let per_item = elapsed / self.done as u32;
            let remaining = per_item * (self.total - self.done) as u32;
            format!("ETA {}", format_duration(remaining))
        } else {
            "ETA --:--".to_string()
        };
        let frame = SPINNER_FRAMES[self.done % SPINNER_FRAMES.len()];
        let running = if self.current_tool.is_empty() {
            String::new()
        } else {
            format!(
                "  {} Running: {}  ({}",
                frame.cyan(),
                self.current_tool.bold(),
                format_elapsed(elapsed)
            )
        };
        let bar_line = format!(
            "  [{}/{}] {}  {}%   {}",
            self.done,
            self.total,
            bar.cyan(),
            pct,
            eta_str.bright_black()
        );
        eprint!(
            "\r{:<width$}",
            bar_line,
            width = self.last_len.max(bar_line.len())
        );
        self.last_len = bar_line.len();
        if !running.is_empty() {
            eprint!("\n{}", running);
            eprint!("\x1b[1A");
        }
        let _ = std::io::Write::flush(&mut std::io::stderr());
    }

    pub fn finish(&self) {
        if self.tty {
            eprintln!("\r{:<80}", "");
        }
    }
}

pub fn format_elapsed(d: std::time::Duration) -> String {
    let total_ms = d.as_millis();
    if total_ms < 1000 {
        format!("{:.1}s", total_ms as f64 / 1000.0)
    } else {
        format!("{:.1}s", d.as_secs_f64())
    }
}

pub fn format_ms(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}

pub fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}
