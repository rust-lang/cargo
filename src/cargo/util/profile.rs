use std::env;
use std::fmt;
use std::mem;
use std::time;
use std::iter::repeat;
use std::cell::RefCell;

thread_local!(static PROFILE_STACK: RefCell<Vec<time::Instant>> = RefCell::new(Vec::new()));
thread_local!(static MESSAGES: RefCell<Vec<Message>> = RefCell::new(Vec::new()));

type Message = (usize, u64, String);

pub struct Profiler {
    desc: String,
}

fn enabled_level() -> Option<usize> {
    env::var("CARGO_PROFILE").ok().and_then(|s| s.parse().ok())
}

pub fn start<T: fmt::Display>(desc: T) -> Profiler {
    if enabled_level().is_none() { return Profiler { desc: String::new() } }

    PROFILE_STACK.with(|stack| stack.borrow_mut().push(time::Instant::now()));

    Profiler {
        desc: desc.to_string(),
    }
}

impl Drop for Profiler {
    fn drop(&mut self) {
        let enabled = match enabled_level() {
            Some(i) => i,
            None => return,
        };

        let start = PROFILE_STACK.with(|stack| stack.borrow_mut().pop().unwrap());
        let duration = start.elapsed();
        let duration_ms = duration.as_secs() * 1000 + u64::from(duration.subsec_nanos() / 1_000_000);

        let stack_len = PROFILE_STACK.with(|stack| stack.borrow().len());
        if stack_len == 0 {
            fn print(lvl: usize, msgs: &[Message], enabled: usize) {
                if lvl > enabled { return }
                let mut last = 0;
                for (i, &(l, time, ref msg)) in msgs.iter().enumerate() {
                    if l != lvl { continue }
                    println!("{} {:6}ms - {}",
                             repeat("    ").take(lvl + 1).collect::<String>(),
                             time, msg);

                    print(lvl + 1, &msgs[last..i], enabled);
                    last = i;
                }

            }
            MESSAGES.with(|msgs_rc| {
                let mut msgs = msgs_rc.borrow_mut();
                msgs.push((0, duration_ms,
                           mem::replace(&mut self.desc, String::new())));
                print(0, &msgs, enabled);
            });
        } else {
            MESSAGES.with(|msgs| {
                let msg = mem::replace(&mut self.desc, String::new());
                msgs.borrow_mut().push((stack_len, duration_ms, msg));
            });
        }
    }
}
