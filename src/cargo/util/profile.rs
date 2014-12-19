use std::os;
use std::mem;
use std::fmt::Show;
use time;
use std::cell::RefCell;

thread_local!(static PROFILE_STACK: RefCell<Vec<u64>> = RefCell::new(Vec::new()));
thread_local!(static MESSAGES: RefCell<Vec<Message>> = RefCell::new(Vec::new()));

type Message = (uint, u64, String);

pub struct Profiler {
    desc: String,
}

fn enabled() -> bool { os::getenv("CARGO_PROFILE").is_some() }

pub fn start<T: Show>(desc: T) -> Profiler {
    if !enabled() { return Profiler { desc: String::new() } }

    PROFILE_STACK.with(|stack| stack.borrow_mut().push(time::precise_time_ns()));

    Profiler {
        desc: desc.to_string(),
    }
}

impl Drop for Profiler {
    fn drop(&mut self) {
        if !enabled() { return }

        let start = PROFILE_STACK.with(|stack| stack.borrow_mut().pop().unwrap());
        let end = time::precise_time_ns();

        let stack_len = PROFILE_STACK.with(|stack| stack.borrow().len());
        if stack_len == 0 {
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
            MESSAGES.with(|msgs_rc| {
                let mut msgs = msgs_rc.borrow_mut();
                msgs.push((0, end - start, mem::replace(&mut self.desc, String::new())));
                print(0, msgs.as_slice());
            });
        } else {
            MESSAGES.with(|msgs| {
                let msg = mem::replace(&mut self.desc, String::new());
                msgs.borrow_mut().push((stack_len, end - start, msg));
            });
        }
    }
}
