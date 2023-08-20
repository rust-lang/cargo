use std::io;

pub struct State(imp::State);

impl State {
    /// Captures the current state of all CPUs on the system.
    ///
    /// The `State` returned here isn't too meaningful in terms of
    /// interpretation across platforms, but it can be compared to previous
    /// states to get a meaningful cross-platform number.
    pub fn current() -> io::Result<State> {
        imp::current().map(State)
    }

    /// Returns the percentage of time CPUs were idle from the current state
    /// relative to the previous state, as a percentage from 0.0 to 100.0.
    ///
    /// This function will return, as a percentage, the amount of time that the
    /// entire system was idle between the `previous` state and this own state.
    /// This can be useful to compare two snapshots in time of CPU usage to see
    /// how the CPU usage compares between the two.
    pub fn idle_since(&self, previous: &State) -> f64 {
        imp::pct_idle(&previous.0, &self.0)
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::{fs, io};

    pub struct State {
        user: u64,
        nice: u64,
        system: u64,
        idle: u64,
        iowait: u64,
        irq: u64,
        softirq: u64,
        steal: u64,
        guest: u64,
        guest_nice: u64,
    }

    pub fn current() -> io::Result<State> {
        let state = fs::read_to_string("/proc/stat")?;

        (|| {
            let mut parts = state.lines().next()?.split_whitespace();
            if parts.next()? != "cpu" {
                return None;
            }
            Some(State {
                user: parts.next()?.parse::<u64>().ok()?,
                nice: parts.next()?.parse::<u64>().ok()?,
                system: parts.next()?.parse::<u64>().ok()?,
                idle: parts.next()?.parse::<u64>().ok()?,
                iowait: parts.next()?.parse::<u64>().ok()?,
                irq: parts.next()?.parse::<u64>().ok()?,
                softirq: parts.next()?.parse::<u64>().ok()?,
                steal: parts.next()?.parse::<u64>().ok()?,
                guest: parts.next()?.parse::<u64>().ok()?,
                guest_nice: parts.next()?.parse::<u64>().ok()?,
            })
        })()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "first line of /proc/stat malformed"))
    }

    pub fn pct_idle(prev: &State, next: &State) -> f64 {
        let user = next.user - prev.user;
        let nice = next.nice - prev.nice;
        let system = next.system - prev.system;
        let idle = next.idle - prev.idle;
        let iowait = next.iowait.saturating_sub(prev.iowait);
        let irq = next.irq - prev.irq;
        let softirq = next.softirq - prev.softirq;
        let steal = next.steal - prev.steal;
        let guest = next.guest - prev.guest;
        let guest_nice = next.guest_nice - prev.guest_nice;
        let total =
            user + nice + system + idle + iowait + irq + softirq + steal + guest + guest_nice;

        (idle as f64) / (total as f64) * 100.0
    }
}

#[cfg(target_os = "macos")]
#[allow(bad_style)]
mod imp {
    use std::io;
    use std::ptr;

    type host_t = u32;
    type mach_port_t = u32;
    type vm_map_t = mach_port_t;
    type vm_offset_t = usize;
    type vm_size_t = usize;
    type vm_address_t = vm_offset_t;
    type processor_flavor_t = i32;
    type natural_t = u32;
    type processor_info_array_t = *mut i32;
    type mach_msg_type_number_t = i32;
    type kern_return_t = i32;

    const PROESSOR_CPU_LOAD_INFO: processor_flavor_t = 2;
    const CPU_STATE_USER: usize = 0;
    const CPU_STATE_SYSTEM: usize = 1;
    const CPU_STATE_IDLE: usize = 2;
    const CPU_STATE_NICE: usize = 3;
    const CPU_STATE_MAX: usize = 4;

    extern "C" {
        static mut mach_task_self_: mach_port_t;

        fn mach_host_self() -> mach_port_t;
        fn host_processor_info(
            host: host_t,
            flavor: processor_flavor_t,
            out_processor_count: *mut natural_t,
            out_processor_info: *mut processor_info_array_t,
            out_processor_infoCnt: *mut mach_msg_type_number_t,
        ) -> kern_return_t;
        fn vm_deallocate(
            target_task: vm_map_t,
            address: vm_address_t,
            size: vm_size_t,
        ) -> kern_return_t;
    }

    pub struct State {
        user: u64,
        system: u64,
        idle: u64,
        nice: u64,
    }

    #[repr(C)]
    struct processor_cpu_load_info_data_t {
        cpu_ticks: [u32; CPU_STATE_MAX],
    }

    pub fn current() -> io::Result<State> {
        // There's scant little documentation on `host_processor_info`
        // throughout the internet, so this is just modeled after what everyone
        // else is doing. For now this is modeled largely after libuv.

        unsafe {
            let mut num_cpus_u = 0;
            let mut cpu_info = ptr::null_mut();
            let mut msg_type = 0;
            let err = host_processor_info(
                mach_host_self(),
                PROESSOR_CPU_LOAD_INFO,
                &mut num_cpus_u,
                &mut cpu_info,
                &mut msg_type,
            );
            if err != 0 {
                return Err(io::Error::last_os_error());
            }
            let mut ret = State {
                user: 0,
                system: 0,
                idle: 0,
                nice: 0,
            };
            let mut current = cpu_info as *const processor_cpu_load_info_data_t;
            for _ in 0..num_cpus_u {
                ret.user += (*current).cpu_ticks[CPU_STATE_USER] as u64;
                ret.system += (*current).cpu_ticks[CPU_STATE_SYSTEM] as u64;
                ret.idle += (*current).cpu_ticks[CPU_STATE_IDLE] as u64;
                ret.nice += (*current).cpu_ticks[CPU_STATE_NICE] as u64;
                current = current.offset(1);
            }
            vm_deallocate(mach_task_self_, cpu_info as vm_address_t, msg_type as usize);
            Ok(ret)
        }
    }

    pub fn pct_idle(prev: &State, next: &State) -> f64 {
        let user = next.user - prev.user;
        let system = next.system - prev.system;
        let idle = next.idle - prev.idle;
        let nice = next.nice - prev.nice;
        let total = user + system + idle + nice;
        (idle as f64) / (total as f64) * 100.0
    }
}

#[cfg(windows)]
mod imp {
    use std::io;
    use std::mem;

    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetSystemTimes;

    pub struct State {
        idle: FILETIME,
        kernel: FILETIME,
        user: FILETIME,
    }

    pub fn current() -> io::Result<State> {
        unsafe {
            let mut ret = mem::zeroed::<State>();
            let r = GetSystemTimes(&mut ret.idle, &mut ret.kernel, &mut ret.user);
            if r != 0 {
                Ok(ret)
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }

    pub fn pct_idle(prev: &State, next: &State) -> f64 {
        fn to_u64(a: &FILETIME) -> u64 {
            ((a.dwHighDateTime as u64) << 32) | (a.dwLowDateTime as u64)
        }

        let idle = to_u64(&next.idle) - to_u64(&prev.idle);
        let kernel = to_u64(&next.kernel) - to_u64(&prev.kernel);
        let user = to_u64(&next.user) - to_u64(&prev.user);
        let total = user + kernel;
        (idle as f64) / (total as f64) * 100.0
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod imp {
    use std::io;

    pub struct State;

    pub fn current() -> io::Result<State> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "unsupported platform to learn CPU state",
        ))
    }

    pub fn pct_idle(_prev: &State, _next: &State) -> f64 {
        unimplemented!()
    }
}
