use std::cmp;
use std::env;
use std::time::{Duration, Instant};

use core::shell::Verbosity;
use util::{CargoResult, Config};

use unicode_width::UnicodeWidthChar;

pub struct Progress<'cfg> {
    state: Option<State<'cfg>>,
}

pub enum ProgressStyle {
    Percentage,
    Ratio,
}

struct State<'cfg> {
    config: &'cfg Config,
    format: Format,
    first: bool,
    last_update: Instant,
    name: String,
    done: bool,
}

struct Format {
    style: ProgressStyle,
    max_width: usize,
    max_print: usize,
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
                format: Format {
                    style,
                    max_width: n,
                    max_print: 80,
                },
                first: true,
                last_update: Instant::now(),
                name: name.to_string(),
                done: false,
            }),
        }
    }

    pub fn disable(&mut self) {
        self.state = None;
    }

    pub fn new(name: &str, cfg: &'cfg Config) -> Progress<'cfg> {
        Self::with_style(name, ProgressStyle::Percentage, cfg)
    }

    pub fn tick(&mut self, cur: usize, max: usize) -> CargoResult<()> {
        match self.state {
            Some(ref mut s) => s.tick(cur, max, "", true),
            None => Ok(()),
        }
    }

    pub fn clear(&mut self) {
        if let Some(ref mut s) = self.state {
            s.clear();
        }
    }

    pub fn tick_now(&mut self, cur: usize, max: usize, msg: &str) -> CargoResult<()> {
        match self.state {
            Some(ref mut s) => s.tick(cur, max, msg, false),
            None => Ok(()),
        }
    }
}

impl<'cfg> State<'cfg> {
    fn tick(&mut self, cur: usize, max: usize, msg: &str, throttle: bool) -> CargoResult<()> {
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

        if cur == max {
            self.done = true;
        }

        // Write out a pretty header, then the progress bar itself, and then
        // return back to the beginning of the line for the next print.
        self.try_update_max_width();
        if let Some(string) = self.format.progress_status(cur, max, msg) {
            self.config.shell().status_header(&self.name)?;
            write!(self.config.shell().err(), "{}\r", string)?;
        }
        Ok(())
    }

    fn clear(&mut self) {
        self.try_update_max_width();
        let blank = " ".repeat(self.format.max_width);
        drop(write!(self.config.shell().err(), "{}\r", blank));
    }

    fn try_update_max_width(&mut self) {
        if let Some(n) = self.config.shell().err_width() {
            self.format.max_width = n;
        }
    }
}

impl Format {
    fn progress_status(&self, cur: usize, max: usize, msg: &str) -> Option<String> {
        // Render the percentage at the far right and then figure how long the
        // progress bar is
        let pct = (cur as f64) / (max as f64);
        let pct = if !pct.is_finite() { 0.0 } else { pct };
        let stats = match self.style {
            ProgressStyle::Percentage => format!(" {:6.02}%", pct * 100.0),
            ProgressStyle::Ratio => format!(" {}/{}", cur, max),
        };
        let extra_len = stats.len() + 2 /* [ and ] */ + 15 /* status header */;
        let display_width = match self.width().checked_sub(extra_len) {
            Some(n) => n,
            None => return None,
        };

        let mut string = String::with_capacity(self.max_width);
        string.push('[');
        let hashes = display_width as f64 * pct;
        let hashes = hashes as usize;

        // Draw the `===>`
        if hashes > 0 {
            for _ in 0..hashes - 1 {
                string.push_str("=");
            }
            if cur == max {
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

        let mut avail_msg_len = self.max_width - self.width();
        let mut ellipsis_pos = 0;
        if avail_msg_len > 3 {
            for c in msg.chars() {
                let display_width = c.width().unwrap_or(0);
                if avail_msg_len >= display_width {
                    avail_msg_len -= display_width;
                    string.push(c);
                    if avail_msg_len >= 3 {
                        ellipsis_pos = string.len();
                    }
                } else {
                    string.truncate(ellipsis_pos);
                    string.push_str("...");
                    break;
                }
            }
        }

        Some(string)
    }

    fn width(&self) -> usize {
        cmp::min(self.max_width, self.max_print)
    }
}

impl<'cfg> Drop for State<'cfg> {
    fn drop(&mut self) {
        self.clear();
    }
}

#[test]
fn test_progress_status() {
    let format = Format {
        style: ProgressStyle::Ratio,
        max_print: 40,
        max_width: 60,
    };
    assert_eq!(
        format.progress_status(0, 4, ""),
        Some("[                   ] 0/4".to_string())
    );
    assert_eq!(
        format.progress_status(1, 4, ""),
        Some("[===>               ] 1/4".to_string())
    );
    assert_eq!(
        format.progress_status(2, 4, ""),
        Some("[========>          ] 2/4".to_string())
    );
    assert_eq!(
        format.progress_status(3, 4, ""),
        Some("[=============>     ] 3/4".to_string())
    );
    assert_eq!(
        format.progress_status(4, 4, ""),
        Some("[===================] 4/4".to_string())
    );

    assert_eq!(
        format.progress_status(3999, 4000, ""),
        Some("[===========> ] 3999/4000".to_string())
    );
    assert_eq!(
        format.progress_status(4000, 4000, ""),
        Some("[=============] 4000/4000".to_string())
    );

    assert_eq!(
        format.progress_status(3, 4, ": short message"),
        Some("[=============>     ] 3/4: short message".to_string())
    );
    assert_eq!(
        format.progress_status(3, 4, ": msg thats just fit"),
        Some("[=============>     ] 3/4: msg thats just fit".to_string())
    );
    assert_eq!(
        format.progress_status(3, 4, ": msg that's just fit"),
        Some("[=============>     ] 3/4: msg that's just...".to_string())
    );

    // combining diacritics have width zero and thus can fit max_width.
    let zalgo_msg = "z̸̧̢̗͉̝̦͍̱ͧͦͨ̑̅̌ͥ́͢a̢ͬͨ̽ͯ̅̑ͥ͋̏̑ͫ̄͢͏̫̝̪̤͎̱̣͍̭̞̙̱͙͍̘̭͚l̶̡̛̥̝̰̭̹̯̯̞̪͇̱̦͙͔̘̼͇͓̈ͨ͗ͧ̓͒ͦ̀̇ͣ̈ͭ͊͛̃̑͒̿̕͜g̸̷̢̩̻̻͚̠͓̞̥͐ͩ͌̑ͥ̊̽͋͐̐͌͛̐̇̑ͨ́ͅo͙̳̣͔̰̠̜͕͕̞̦̙̭̜̯̹̬̻̓͑ͦ͋̈̉͌̃ͯ̀̂͠ͅ ̸̡͎̦̲̖̤̺̜̮̱̰̥͔̯̅̏ͬ̂ͨ̋̃̽̈́̾̔̇ͣ̚͜͜h̡ͫ̐̅̿̍̀͜҉̛͇̭̹̰̠͙̞ẽ̶̙̹̳̖͉͎̦͂̋̓ͮ̔ͬ̐̀͂̌͑̒͆̚͜͠ ͓͓̟͍̮̬̝̝̰͓͎̼̻ͦ͐̾̔͒̃̓͟͟c̮̦͍̺͈͚̯͕̄̒͐̂͊̊͗͊ͤͣ̀͘̕͝͞o̶͍͚͍̣̮͌ͦ̽̑ͩ̅ͮ̐̽̏͗́͂̅ͪ͠m̷̧͖̻͔̥̪̭͉͉̤̻͖̩̤͖̘ͦ̂͌̆̂ͦ̒͊ͯͬ͊̉̌ͬ͝͡e̵̹̣͍̜̺̤̤̯̫̹̠̮͎͙̯͚̰̼͗͐̀̒͂̉̀̚͝͞s̵̲͍͙͖̪͓͓̺̱̭̩̣͖̣ͤͤ͂̎̈͗͆ͨͪ̆̈͗͝͠";
    assert_eq!(
        format.progress_status(3, 4, zalgo_msg),
        Some("[=============>     ] 3/4".to_string() + zalgo_msg)
    );

    // some non-ASCII ellipsize test
    assert_eq!(
        format.progress_status(3, 4, "_123456789123456e\u{301}\u{301}8\u{301}90a"),
        Some("[=============>     ] 3/4_123456789123456e\u{301}\u{301}...".to_string())
    );
    assert_eq!(
        format.progress_status(3, 4, "：每個漢字佔據了兩個字元"),
        Some("[=============>     ] 3/4：每個漢字佔據了...".to_string())
    );
}

#[test]
fn test_progress_status_percentage() {
    let format = Format {
        style: ProgressStyle::Percentage,
        max_print: 40,
        max_width: 60,
    };
    assert_eq!(
        format.progress_status(0, 77, ""),
        Some("[               ]   0.00%".to_string())
    );
    assert_eq!(
        format.progress_status(1, 77, ""),
        Some("[               ]   1.30%".to_string())
    );
    assert_eq!(
        format.progress_status(76, 77, ""),
        Some("[=============> ]  98.70%".to_string())
    );
    assert_eq!(
        format.progress_status(77, 77, ""),
        Some("[===============] 100.00%".to_string())
    );
}

#[test]
fn test_progress_status_too_short() {
    let format = Format {
        style: ProgressStyle::Percentage,
        max_print: 25,
        max_width: 25,
    };
    assert_eq!(
        format.progress_status(1, 1, ""),
        Some("[] 100.00%".to_string())
    );

    let format = Format {
        style: ProgressStyle::Percentage,
        max_print: 24,
        max_width: 24,
    };
    assert_eq!(
        format.progress_status(1, 1, ""),
        None
    );
}
