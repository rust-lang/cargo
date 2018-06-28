use std::cmp;
use std::env;
use std::time::{Duration, Instant};

use core::shell::Verbosity;
use util::{CargoResult, Config};

pub struct Progress<'cfg> {
    state: Option<State<'cfg>>,
}

pub enum ProgressStyle {
    Percentage,
    Ratio,
}

struct State<'cfg> {
    config: &'cfg Config,
    style: ProgressStyle,
    max_width: usize,
    width: usize,
    first: bool,
    last_update: Instant,
    name: String,
    done: bool,
}

impl<'cfg> Progress<'cfg> {
    pub fn with_style(name: &str, style: ProgressStyle, cfg: &'cfg Config) -> Progress<'cfg> {
        // report no progress when -q (for quiet) or TERM=dumb are set
        let dumb = match env::var("TERM") {
            Ok(term) => term == "dumb",
            Err(_) => false,
        };
        if cfg.shell().verbosity() == Verbosity::Quiet || dumb {
            return Progress { state: None };
        }

        Progress {
            state: cfg.shell().err_width().map(|n| State {
                config: cfg,
                style,
                max_width: n,
                width: cmp::min(n, 80),
                first: true,
                last_update: Instant::now(),
                name: name.to_string(),
                done: false,
            }),
        }
    }

    pub fn new(name: &str, cfg: &'cfg Config) -> Progress<'cfg> {
        Self::with_style(name, ProgressStyle::Percentage, cfg)
    }

    pub fn tick(&mut self, cur: usize, max: usize) -> CargoResult<()> {
        match self.state {
            Some(ref mut s) => s.tick(cur, max, String::new(), true),
            None => Ok(()),
        }
    }

    pub fn clear(&mut self) {
        if let Some(ref mut s) = self.state {
            clear(s.max_width, s.config);
        }
    }

    pub fn tick_now(&mut self, cur: usize, max: usize, msg: String) -> CargoResult<()> {
        match self.state {
            Some(ref mut s) => s.tick(cur, max, msg, false),
            None => Ok(()),
        }
    }
}

impl<'cfg> State<'cfg> {
    fn tick(&mut self, cur: usize, max: usize, msg: String, throttle: bool) -> CargoResult<()> {
        if self.done {
            return Ok(());
        }

        // Don't update too often as it can cause excessive performance loss
        // just putting stuff onto the terminal. We also want to avoid
        // flickering by not drawing anything that goes away too quickly. As a
        // result we've got two branches here:
        //
        // 1. If we haven't drawn anything, we wait for a period of time to
        //    actually start drawing to the console. This ensures that
        //    short-lived operations don't flicker on the console. Currently
        //    there's a 500ms delay to when we first draw something.
        // 2. If we've drawn something, then we rate limit ourselves to only
        //    draw to the console every so often. Currently there's a 100ms
        //    delay between updates.
        if throttle {
            if self.first {
                let delay = Duration::from_millis(500);
                if self.last_update.elapsed() < delay {
                    return Ok(());
                }
                self.first = false;
            } else {
                let interval = Duration::from_millis(100);
                if self.last_update.elapsed() < interval {
                    return Ok(());
                }
            }
            self.last_update = Instant::now();
        }

        // Render the percentage at the far right and then figure how long the
        // progress bar is
        let pct = (cur as f64) / (max as f64);
        let pct = if !pct.is_finite() { 0.0 } else { pct };
        let stats = match self.style {
            ProgressStyle::Percentage => format!(" {:6.02}%", pct * 100.0),
            ProgressStyle::Ratio => format!(" {}/{}", cur, max),
        };
        let extra_len = stats.len() + 2 /* [ and ] */ + 15 /* status header */;
        let display_width = match self.width.checked_sub(extra_len) {
            Some(n) => n,
            None => return Ok(()),
        };
        let mut string = String::from("[");
        let hashes = display_width as f64 * pct;
        let hashes = hashes as usize;

        // Draw the `===>`
        if hashes > 0 {
            for _ in 0..hashes - 1 {
                string.push_str("=");
            }
            if cur == max {
                self.done = true;
                string.push_str("=");
            } else {
                string.push_str(">");
            }
        }

        // Draw the empty space we have left to do
        for _ in 0..(display_width - hashes) {
            string.push_str(" ");
        }
        string.push_str("]");
        string.push_str(&stats);

        let avail_msg_len = self.max_width - self.width;
        if avail_msg_len >= msg.len() + 3 {
            string.push_str(&msg);
        } else if avail_msg_len >= 4 {
            string.push_str(&msg[..(avail_msg_len - 3)]);
            string.push_str("...");
        }

        // Write out a pretty header, then the progress bar itself, and then
        // return back to the beginning of the line for the next print.
        self.config.shell().status_header(&self.name)?;
        write!(self.config.shell().err(), "{}\r", string)?;
        Ok(())
    }
}

fn clear(width: usize, config: &Config) {
    let blank = " ".repeat(width);
    drop(write!(config.shell().err(), "{}\r", blank));
}

impl<'cfg> Drop for State<'cfg> {
    fn drop(&mut self) {
        clear(self.max_width, self.config);
    }
}
