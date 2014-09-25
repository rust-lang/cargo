use std::os;
use std::mem;
use std::fmt::Show;
use time;

local_data_key!(PROFILE_STACK: Vec<u64>)
local_data_key!(MESSAGES: Vec<Message>)

type Message = (uint, u64, String);

pub struct Profiler {
    desc: String,
}

fn enabled() -> bool { os::getenv("CARGO_PROFILE").is_some() }

pub fn start<T: Show>(desc: T) -> Profiler {
    if !enabled() { return Profiler { desc: String::new() } }

    let mut stack = PROFILE_STACK.replace(None).unwrap_or(Vec::new());
    stack.push(time::precise_time_ns());
    PROFILE_STACK.replace(Some(stack));

    Profiler {
        desc: desc.to_string(),
    }
}

impl Drop for Profiler {
    fn drop(&mut self) {
        if !enabled() { return }

        let mut stack = PROFILE_STACK.replace(None).unwrap_or(Vec::new());
        let mut msgs = MESSAGES.replace(None).unwrap_or(Vec::new());

        let start = stack.pop().unwrap();
        let end = time::precise_time_ns();

        let msg = mem::replace(&mut self.desc, String::new());
        if stack.len() == 0 {
            fn print(lvl: uint, msgs: &[Message]) {
                let mut last = 0;
                for (i, &(l, time, ref msg)) in msgs.iter().enumerate() {
                    if l != lvl { continue }
                    println!("{} {:6}ms - {}", "    ".repeat(lvl + 1),
                             time / 1000000, msg);

                    print(lvl + 1, msgs.slice(last, i));
                    last = i;
                }

            }
            msgs.push((0, end - start, msg));
            print(0, msgs.as_slice());
        } else {
            msgs.push((stack.len(), end - start, msg));
            MESSAGES.replace(Some(msgs));
        }
        PROFILE_STACK.replace(Some(stack));

    }
}
