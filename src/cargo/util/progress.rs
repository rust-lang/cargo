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

struct Throttle {
    first: bool,
    last_update: Instant,
}

struct State<'cfg> {
    config: &'cfg Config,
    format: Format,
    name: String,
    done: bool,
    throttle: Throttle,
}

struct Format {
    formatting: String,
    max_width: usize,
    max_print: usize,
}

impl<'cfg> Progress<'cfg> {
    pub fn with_custom_style(name: &str, cfg: &'cfg Config) -> Progress<'cfg> {
   // @TODO: use   let format_template = cfg.get_string("TERM").unwrap().map(|s| s.val);
        let format_template: String = match env::var("CARGO_STATUS") {
            Ok(template) => template,
            // if not set, fall back to default
            Err(_) => String::from("[%b] %s/%t: %n"),
        };

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
                    formatting: format_template,
                    max_width: n,
                    max_print: 80,
                },
                name: name.to_string(),
                done: false,
                throttle: Throttle::new(),
            }),
        }
    }

    pub fn with_ratio(name: &str, cfg: &'cfg Config) -> Progress<'cfg> {
        // default compile progress, ratio style, which cannot be overridden

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
                    formatting: "[%b], %s/t%n".to_string(),
                    max_width: n,
                    max_print: 80,
                },
                name: name.to_string(),
                done: false,
                throttle: Throttle::new(),
            }),
        }
    }

    pub fn with_percentage(name: &str, cfg: &'cfg Config) -> Progress<'cfg> {
        // default compile progress, percentage style, which cannot be overridden

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
                    formatting: "[%b] %p%n".to_string(),
                    max_width: n,
                    max_print: 80,
                },
                name: name.to_string(),
                done: false,
                throttle: Throttle::new(),
            }),
        }
    }


    pub fn disable(&mut self) {
        self.state = None;
    }

    pub fn is_enabled(&self) -> bool {
        self.state.is_some()
    }

    pub fn new(name: &str, cfg: &'cfg Config) -> Progress<'cfg> {
        Self::with_custom_style(name, cfg)
    }

    pub fn tick(&mut self, cur: usize, max: usize, active: usize, render_jobs: bool) -> CargoResult<()> {
        let s = match &mut self.state {
            Some(s) => s,
            None => return Ok(()),
        };

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
        if !s.throttle.allowed() {
            return Ok(())
        }

        s.tick(cur, max, active, "", render_jobs)
    }

    pub fn tick_now(&mut self, cur: usize, max: usize, active_names: Vec<String>, render_jobs: bool) -> CargoResult<()> {
        let active = active_names.len();
        let msg = &active_names.join(", ");
        match self.state {
            Some(ref mut s) => s.tick(cur, max, active, msg, render_jobs),
            None => Ok(()),
        }
    }

    pub fn update_allowed(&mut self) -> bool {
        match &mut self.state {
            Some(s) => s.throttle.allowed(),
            None => false,
        }
    }

    pub fn print_now(&mut self, msg: &str, render_jobs: bool) -> CargoResult<()> {
        match &mut self.state {
            Some(s) => s.print("", msg, render_jobs),
            None => Ok(()),
        }
    }

    pub fn clear(&mut self) {
        if let Some(ref mut s) = self.state {
            s.clear();
        }
    }
}

impl Throttle {
    fn new() -> Throttle {
        Throttle {
            first: true,
            last_update: Instant::now(),
        }
    }

    fn allowed(&mut self) -> bool {
        if self.first {
            let delay = Duration::from_millis(500);
            if self.last_update.elapsed() < delay {
                return false
            }
        } else {
            let interval = Duration::from_millis(100);
            if self.last_update.elapsed() < interval {
                return false
            }
        }
        self.update();
        true
    }

    fn update(&mut self) {
        self.first = false;
        self.last_update = Instant::now();
    }
}

impl<'cfg> State<'cfg> {
    fn tick(&mut self, cur: usize, max: usize, active: usize, msg: &str, render_jobs: bool) -> CargoResult<()> {
        if self.done {
            return Ok(());
        }

        if max > 0 && cur == max {
            self.done = true;
        }

        // Write out a pretty header, then the progress bar itself, and then
        // return back to the beginning of the line for the next print.
        self.try_update_max_width();
        if let Some(pbar) = self.format.progress(cur, max, active, msg) {
            self.print(&pbar, msg, render_jobs)?;
        }
        Ok(())
    }

    fn print(&mut self, prefix: &str, msg: &str, render_jobs: bool) -> CargoResult<()> {
        self.throttle.update();
        self.try_update_max_width();

        // make sure we have enough room for the header
        if self.format.max_width < 15 {
            return Ok(())
        }
        self.config.shell().status_header(&self.name)?;
        let mut line = prefix.to_string();
        if render_jobs {
            // this is only needed for displaying download status, otherwise it will interfere
            // with customizable compile progress
            self.format.render(&mut line, msg);
        }
        while line.len() < self.format.max_width - 15 {
            line.push(' ');
        }

        write!(self.config.shell().err(), "{}\r", line)?;
        Ok(())
    }

    fn clear(&mut self) {
        self.config.shell().err_erase_line();
    }

    fn try_update_max_width(&mut self) {
        if let Some(n) = self.config.shell().err_width() {
            self.format.max_width = n;
        }
    }
}

impl Format {
    fn progress(&self, cur: usize, max: usize, _active: usize, msg: &str) -> Option<String> {
        // %b progress bar
        // %s number of done jobs
        // %t number of total jobs
        // %p progress percentage
        // %n list of names of running jobs

        let template = &self.formatting; // what the formatting is supposed to look like
         // what is left if we remove all dynamic parameters
         // will be "[] /" for default formatting of "[%b] %s/%t%n"
        let mut template_skelleton = template.to_string();
        for fmt in &["%b", "%s", "%t", "%p", "%n"] {
            // remove all the formatting specifiers
            template_skelleton = template_skelleton.replace(fmt, "");
        }
        let length_of_formatters = template.len() - template_skelleton.len();

        // Render the percentage at the far right and then figure how long the progress bar iscustom.compile_progress
        let pct = (cur as f64) / (max as f64);
        let pct = if !pct.is_finite() { 0.0 } else { pct };

        let percentage = if template.contains("%p") { format!("{:6.02}%", pct * 100.0) } else { String::new() };
        // compile status default looks like this
        //      Building [=====>                    ] 30/128,
        // |____________|||________________________||| ||    \_
        // status header |    progress bar          `|cur`fmt  max
        // passed via formatting          both passed via formatting
        let cur_str = if template.contains("%s") { cur.to_string() } else { String::new() };
        let max_str = if template.contains("%t") { max.to_string() } else { String::new() };
        const STATUS_HEADER_LEN: usize = 15;
        // extra_len is everything without the progress bar and without the jobs_names
        let extra_len =  STATUS_HEADER_LEN + percentage.len() + cur_str.len() + max_str.len()  + /* all other fmt chars: */ template_skelleton.len();
        // display_width will determine the length of the progress bar

        let display_width = match self.width().checked_sub(extra_len) {
            Some(n) => n,
            None => return None,
        };

        let mut progress_bar = String::with_capacity(self.max_width);
        let hashes = display_width as f64 * pct;
        let hashes = hashes as usize;

        // Draw the `===>`
        if hashes > 0 {
            progress_bar.push_str(&"=".repeat(hashes-1));
            if cur == max {
                progress_bar.push_str("=");
            } else {
                progress_bar.push_str(">");
            }
        }

        // Draw the empty space we have left to do
        progress_bar.push_str(&" ".repeat(display_width - hashes));

        let mut jobs_names = String::new();
        let mut avail_msg_len = self.max_width - (
                progress_bar.len()
                + template_skelleton.len()
                + length_of_formatters /* <- since we replace these */
                + percentage.len()
                + cur_str.len()
                + max_str.len()
                + 7  /* ?? */);

        let mut ellipsis_pos = 0;
        if avail_msg_len > 3 {
            for c in msg.chars() {
                let display_width = c.width().unwrap_or(0);
                if avail_msg_len >= display_width {
                    avail_msg_len -= display_width;
                    jobs_names.push(c);
                    if avail_msg_len >= 3 {
                        ellipsis_pos = jobs_names.len();
                    }
                } else {
                    jobs_names.truncate(ellipsis_pos);
                    jobs_names.push_str("...");
                    break;
                }
            }
        }
        let mut string = template.clone();
        string = string.replace("%s", &cur_str);
        string = string.replace("%t", &max_str);
        string = string.replace("%p", &percentage);
        string = string.replace("%b", &progress_bar);
        string = string.replace("%n", &jobs_names);


        Some(string)
    }

    #[inline]
    fn render(&self, string: &mut String, msg: &str) {
        let mut avail_msg_len = self.max_width - string.len() - 15;
        let mut ellipsis_pos = 0;
        if avail_msg_len <= 3 {
            return
        }
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

    #[cfg(test)]
    fn progress_status(&self, cur: usize, max: usize, active: usize, msg: &str) -> Option<String> {
        let ret = self.progress(cur, max, active, msg)?;
        Some(ret)
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
        formatting: "[%b] %s/%t: %n".to_string(),
        max_print: 42,
        max_width: 60,
    };
    assert_eq!(
        format.progress_status(0, 4, 1, ""),
        Some("[                   ] 0/4: ".to_string())
    );
    assert_eq!(
        format.progress_status(1, 4, 1, ""),
        Some("[===>               ] 1/4: ".to_string())
    );
    assert_eq!(
        format.progress_status(2, 4, 1, ""),
        Some("[========>          ] 2/4: ".to_string())
    );
    assert_eq!(
        format.progress_status(3, 4, 1, ""),
        Some("[=============>     ] 3/4: ".to_string())
    );
    assert_eq!(
        format.progress_status(4, 4, 0, ""),
        Some("[===================] 4/4: ".to_string())
    );

    assert_eq!(
        format.progress_status(3999, 4000, 1, ""),
        Some("[===========> ] 3999/4000: ".to_string())
    );
    assert_eq!(
        format.progress_status(4000, 4000, 0, ""),
        Some("[=============] 4000/4000: ".to_string())
    );

    assert_eq!(
        format.progress_status(3, 4, 1, "short message"),
        Some("[=============>     ] 3/4: short message".to_string())
    );
    assert_eq!(
        format.progress_status(3, 4, 1, "msg thats just fit"),
        Some("[=============>     ] 3/4: msg thats just fit".to_string())
    );
    assert_eq!(
        format.progress_status(3, 4, 1, "msg that's just fit"),
        Some("[=============>     ] 3/4: msg that's just...".to_string())
    );

    // combining diacritics have width zero and thus can fit max_width.
    let zalgo_msg = "z̸̧̢̗͉̝̦͍̱ͧͦͨ̑̅̌ͥ́͢a̢ͬͨ̽ͯ̅̑ͥ͋̏̑ͫ̄͢͏̫̝̪̤͎̱̣͍̭̞̙̱͙͍̘̭͚l̶̡̛̥̝̰̭̹̯̯̞̪͇̱̦͙͔̘̼͇͓̈ͨ͗ͧ̓͒ͦ̀̇ͣ̈ͭ͊͛̃̑͒̿̕͜g̸̷̢̩̻̻͚̠͓̞̥͐ͩ͌̑ͥ̊̽͋͐̐͌͛̐̇̑ͨ́ͅo͙̳̣͔̰̠̜͕͕̞̦̙̭̜̯̹̬̻̓͑ͦ͋̈̉͌̃ͯ̀̂͠ͅ ̸̡͎̦̲̖̤̺̜̮̱̰̥͔̯̅̏ͬ̂ͨ̋̃̽̈́̾̔̇ͣ̚͜͜h̡ͫ̐̅̿̍̀͜҉̛͇̭̹̰̠͙̞ẽ̶̙̹̳̖͉͎̦͂̋̓ͮ̔ͬ̐̀͂̌͑̒͆̚͜͠ ͓͓̟͍̮̬̝̝̰͓͎̼̻ͦ͐̾̔͒̃̓͟͟c̮̦͍̺͈͚̯͕̄̒͐̂͊̊͗͊ͤͣ̀͘̕͝͞o̶͍͚͍̣̮͌ͦ̽̑ͩ̅ͮ̐̽̏͗́͂̅ͪ͠m̷̧͖̻͔̥̪̭͉͉̤̻͖̩̤͖̘ͦ̂͌̆̂ͦ̒͊ͯͬ͊̉̌ͬ͝͡e̵̹̣͍̜̺̤̤̯̫̹̠̮͎͙̯͚̰̼͗͐̀̒͂̉̀̚͝͞s̵̲͍͙͖̪͓͓̺̱̭̩̣͖̣ͤͤ͂̎̈͗͆ͨͪ̆̈͗͝͠";
    assert_eq!(
        format.progress_status(3, 4, 1, zalgo_msg),
        Some("[=============>     ] 3/4: ".to_string() + zalgo_msg)
    );

    // some non-ASCII ellipsize test
    assert_eq!(
        format.progress_status(3, 4, 1, "_1234567891234e\u{301}\u{301}8\u{301}90a"),
        Some("[=============>     ] 3/4: _1234567891234e\u{301}\u{301}...".to_string())
    );
    assert_eq!(
        format.progress_status(3, 4, 1, "每個漢字佔據了兩個字元"),
        Some("[=============>     ] 3/4: 每個漢字佔據了...".to_string())
    );
}

#[test]
fn test_progress_status_percentage() {
    let format = Format {
        formatting: "[%b] %p: %n".to_string(),
        max_print: 42,
        max_width: 60,
    };
    assert_eq!(
        format.progress_status(0, 77, 1, ""),
        Some("[               ]   0.00%: ".to_string())
    );
    assert_eq!(
        format.progress_status(1, 77, 1, ""),
        Some("[               ]   1.30%: ".to_string())
    );
    assert_eq!(
        format.progress_status(76, 77, 1, ""),
        Some("[=============> ]  98.70%: ".to_string())
    );
    assert_eq!(
        format.progress_status(77, 77, 1, ""),
        Some("[===============] 100.00%: ".to_string())
    );
}

#[test]
fn test_progress_status_too_short() {
    let format = Format {
        formatting: "[%b] %p: %n".to_string(),
        max_print: 27,
        max_width: 27,
    };
    assert_eq!(
        format.progress_status(1, 1, 0, ""),
        Some("[] 100.00%: ".to_string())
    );

    let format = Format {
        formatting: "[%b] %p: %n".to_string(),
        max_print: 26,
        max_width: 26,
    };
    assert_eq!(
        format.progress_status(1, 1, 1, ""),
        None
    );
}
