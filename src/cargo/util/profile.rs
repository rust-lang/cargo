//! An internal performance profiler for Cargo itself.
//!
//! > **Note**: This might not be the module you are looking for.
//! > For information about how Cargo handles compiler flags with profiles,
//! > please see the module [`cargo::core::profiles`](crate::core::profiles).

use std::cell::RefCell;
use std::env;
use std::fmt;
use std::io::{stdout, StdoutLock, Write};
use std::iter::repeat;
use std::mem;
use std::time;

thread_local!(static PROFILE_STACK: RefCell<Vec<time::Instant>> = RefCell::new(Vec::new()));
thread_local!(static MESSAGES: RefCell<Vec<Message>> = RefCell::new(Vec::new()));

type Message = (usize, u64, String);

pub struct Profiler {
    desc: String,
}

fn enabled_level() -> Option<usize> {
    // ALLOWED: for profiling Cargo itself, not intended to be used beyond Cargo contributors.
    #[allow(clippy::disallowed_methods)]
    env::var("CARGO_PROFILE").ok().and_then(|s| s.parse().ok())
}

pub fn start<T: fmt::Display>(desc: T) -> Profiler {
    if enabled_level().is_none() {
        return Profiler {
            desc: String::new(),
        };
    }

    PROFILE_STACK.with(|stack| stack.borrow_mut().push(time::Instant::now()));

    Profiler {
        desc: desc.to_string(),
    }
}

impl Drop for Profiler {
    fn drop(&mut self) {
        let Some(enabled) = enabled_level() else {
            return;
        };

        let (start, stack_len) = PROFILE_STACK.with(|stack| {
            let mut stack = stack.borrow_mut();
            let start = stack.pop().unwrap();
            (start, stack.len())
        });
        let duration = start.elapsed();
        let duration_ms = duration.as_secs() * 1000 + u64::from(duration.subsec_millis());

        let msg = (stack_len, duration_ms, mem::take(&mut self.desc));
        MESSAGES.with(|msgs| msgs.borrow_mut().push(msg));

        if stack_len == 0 {
            fn print(lvl: usize, msgs: &[Message], enabled: usize, stdout: &mut StdoutLock<'_>) {
                if lvl > enabled {
                    return;
                }
                let mut last = 0;
                for (i, &(l, time, ref msg)) in msgs.iter().enumerate() {
                    if l != lvl {
                        continue;
                    }
                    writeln!(
                        stdout,
                        "{} {:6}ms - {}",
                        repeat("    ").take(lvl + 1).collect::<String>(),
                        time,
                        msg
                    )
                    .expect("printing profiling info to stdout");

                    print(lvl + 1, &msgs[last..i], enabled, stdout);
                    last = i;
                }
            }
            let stdout = stdout();
            MESSAGES.with(|msgs| {
                let mut msgs = msgs.borrow_mut();
                print(0, &msgs, enabled, &mut stdout.lock());
                msgs.clear();
            });
        }
    }
}
