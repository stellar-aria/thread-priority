//! This module defines the Haiku thread control.
//!
//! Haiku uses a native threading API that differs from the POSIX pthread
//! scheduling functions. This module provides thread priority control
//! using Haiku's native kernel kit functions: `find_thread`, `set_thread_priority`,
//! and `get_thread_info`.

use crate::{Error, ThreadPriority, ThreadPriorityValue};

/// An alias type for a thread id.
/// On Haiku, we use the native `thread_id` type (i32) rather than pthread_t.
pub type ThreadId = i32;

// Haiku's thread_info structure from OS.h
#[repr(C)]
struct ThreadInfo {
    thread: i32,
    team: i32,
    name: [libc::c_char; 32], // B_OS_NAME_LENGTH
    state: i32,               // thread_state enum
    priority: i32,
    sem: i32,
    user_time: i64,           // bigtime_t
    kernel_time: i64,         // bigtime_t
    stack_base: *mut libc::c_void,
    stack_end: *mut libc::c_void,
}

// Link to libroot.so for Haiku kernel kit functions
#[link(name = "root")]
unsafe extern "C" {
    fn find_thread(name: *const libc::c_char) -> i32;
    fn set_thread_priority(thread: i32, new_priority: i32) -> i32;
    // Note: get_thread_info is a macro in Haiku, the actual function is _get_thread_info
    fn _get_thread_info(id: i32, info: *mut ThreadInfo, size: libc::size_t) -> i32;
}

/// Minimum thread priority value on Haiku
pub const HAIKU_MIN_PRIORITY: i32 = 0;
/// Maximum thread priority value on Haiku
pub const HAIKU_MAX_PRIORITY: i32 = 120;
/// Default/normal thread priority value on Haiku
pub const HAIKU_NORMAL_PRIORITY: i32 = 10;

/// Proxy structure to maintain compatibility with unix module
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct ScheduleParams {
    /// The thread priority value (Haiku native priority)
    pub sched_priority: libc::c_int,
}

/// The normal (non-realtime) parsing of the scheduling policies.
/// Haiku supports SCHED_OTHER as the primary scheduling policy.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum NormalThreadSchedulePolicy {
    /// The standard round-robin time-sharing policy.
    Other,
}

impl NormalThreadSchedulePolicy {
    fn to_posix(self) -> libc::c_int {
        match self {
            NormalThreadSchedulePolicy::Other => 3, // SCHED_OTHER on Haiku
        }
    }
}

/// Realtime scheduling policies.
/// Note: Haiku's realtime support is limited compared to Linux.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum RealtimeThreadSchedulePolicy {
    /// A first-in, first-out policy
    Fifo,
    /// A round-robin policy
    RoundRobin,
}

impl RealtimeThreadSchedulePolicy {
    fn to_posix(self) -> libc::c_int {
        match self {
            RealtimeThreadSchedulePolicy::Fifo => 1,      // SCHED_FIFO
            RealtimeThreadSchedulePolicy::RoundRobin => 2, // SCHED_RR
        }
    }
}

/// Thread schedule policy definition.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ThreadSchedulePolicy {
    /// Normal (non-realtime) scheduling policies.
    Normal(NormalThreadSchedulePolicy),
    /// Realtime scheduling policies.
    Realtime(RealtimeThreadSchedulePolicy),
}

impl ThreadSchedulePolicy {
    /// Converts to a POSIX policy value (for compatibility)
    pub fn to_posix(self) -> libc::c_int {
        match self {
            ThreadSchedulePolicy::Normal(p) => p.to_posix(),
            ThreadSchedulePolicy::Realtime(p) => p.to_posix(),
        }
    }

    /// Create from a POSIX policy value
    pub fn from_posix(policy: libc::c_int) -> Result<ThreadSchedulePolicy, Error> {
        match policy {
            1 => Ok(ThreadSchedulePolicy::Realtime(RealtimeThreadSchedulePolicy::Fifo)),
            2 => Ok(ThreadSchedulePolicy::Realtime(RealtimeThreadSchedulePolicy::RoundRobin)),
            3 => Ok(ThreadSchedulePolicy::Normal(NormalThreadSchedulePolicy::Other)),
            _ => Ok(ThreadSchedulePolicy::Normal(NormalThreadSchedulePolicy::Other)),
        }
    }
}

/// Returns current thread id using Haiku's native find_thread()
#[inline(always)]
pub fn thread_native_id() -> ThreadId {
    // find_thread(NULL) returns the current thread's thread_id
    unsafe { find_thread(std::ptr::null()) }
}

/// Set thread priority using Haiku's native API
fn set_haiku_thread_priority(thread_id: ThreadId, priority: i32) -> Result<(), Error> {
    let result = unsafe { set_thread_priority(thread_id, priority) };
    
    // set_thread_priority returns the previous priority on success, or an error code (negative)
    if result < 0 {
        Err(Error::OS(result))
    } else {
        Ok(())
    }
}

impl ThreadPriority {
    /// Returns the minimum allowed priority value for a policy.
    pub fn min_value_for_policy(_policy: ThreadSchedulePolicy) -> Result<libc::c_int, Error> {
        Ok(HAIKU_MIN_PRIORITY)
    }

    /// Returns the maximum allowed priority value for a policy.
    pub fn max_value_for_policy(_policy: ThreadSchedulePolicy) -> Result<libc::c_int, Error> {
        Ok(HAIKU_MAX_PRIORITY)
    }

    /// Checks that the passed priority value is within the range of allowed values.
    pub fn to_allowed_value_for_policy(
        priority: libc::c_int,
        policy: ThreadSchedulePolicy,
    ) -> Result<libc::c_int, Error> {
        let min_priority = Self::min_value_for_policy(policy)?;
        let max_priority = Self::max_value_for_policy(policy)?;
        let allowed_range = min_priority..=max_priority;
        
        if allowed_range.contains(&priority) {
            Ok(priority)
        } else {
            Err(Error::PriorityNotInRange(allowed_range))
        }
    }

    /// Converts the priority to a Haiku-compatible value.
    pub fn to_posix(self, policy: ThreadSchedulePolicy) -> Result<libc::c_int, Error> {
        match self {
            ThreadPriority::Min => Self::min_value_for_policy(policy),
            ThreadPriority::Max => Self::max_value_for_policy(policy),
            ThreadPriority::Crossplatform(ThreadPriorityValue(p)) => {
                // Map 0-99 range to Haiku's 0-120 range
                let haiku_priority = (p as i32 * HAIKU_MAX_PRIORITY) / 99;
                Self::to_allowed_value_for_policy(haiku_priority, policy)
            }
            ThreadPriority::Os(crate::ThreadPriorityOsValue(p)) => {
                Self::to_allowed_value_for_policy(p as i32, policy)
            }
        }
    }

    /// Gets priority value from Haiku priority.
    pub fn from_posix(params: ScheduleParams) -> ThreadPriority {
        // Map Haiku's 0-120 range back to 0-99
        let crossplatform = ((params.sched_priority as u32 * 99) / HAIKU_MAX_PRIORITY as u32) as u8;
        ThreadPriority::Crossplatform(ThreadPriorityValue(crossplatform.min(99)))
    }
}

/// Sets thread's priority and schedule policy
pub fn set_thread_priority_and_policy(
    native: ThreadId,
    priority: ThreadPriority,
    policy: ThreadSchedulePolicy,
) -> Result<(), Error> {
    let haiku_priority = priority.to_posix(policy)?;
    set_haiku_thread_priority(native, haiku_priority)
}

/// Set current thread's priority.
pub fn set_current_thread_priority(priority: ThreadPriority) -> Result<(), Error> {
    let thread_id = thread_native_id();
    let policy = thread_schedule_policy()?;
    set_thread_priority_and_policy(thread_id, priority, policy)
}

/// Returns policy parameters for current process
pub fn thread_schedule_policy() -> Result<ThreadSchedulePolicy, Error> {
    // Haiku primarily uses SCHED_OTHER
    Ok(ThreadSchedulePolicy::Normal(NormalThreadSchedulePolicy::Other))
}

/// Returns policy parameters (schedule policy and other schedule parameters)
pub fn thread_schedule_policy_param(
    native: ThreadId,
) -> Result<(ThreadSchedulePolicy, ScheduleParams), Error> {
    let mut info: ThreadInfo = unsafe { std::mem::zeroed() };
    let result = unsafe { _get_thread_info(native, &mut info, std::mem::size_of::<ThreadInfo>()) };
    
    // _get_thread_info returns B_OK (0) on success, negative error code on failure
    if result != 0 {
        return Err(Error::OS(result));
    }
    
    Ok((
        ThreadSchedulePolicy::Normal(NormalThreadSchedulePolicy::Other),
        ScheduleParams {
            sched_priority: info.priority,
        },
    ))
}

/// Get the thread's priority value.
pub fn get_thread_priority(native: ThreadId) -> Result<ThreadPriority, Error> {
    Ok(ThreadPriority::from_posix(
        thread_schedule_policy_param(native)?.1,
    ))
}

/// Get current thread's priority value.
pub fn get_current_thread_priority() -> Result<ThreadPriority, Error> {
    get_thread_priority(thread_native_id())
}

/// A helper trait for other threads to implement to be able to call methods
/// on threads themselves.
pub trait ThreadExt {
    /// Gets the native thread id.
    ///
    /// For more info read [`thread_native_id`].
    ///
    /// ```rust
    /// use thread_priority::*;
    ///
    /// assert!(std::thread::current().get_native_id().unwrap() > 0);
    /// ```
    fn get_native_id(&self) -> Result<ThreadId, Error>;

    /// Gets the current thread's priority.
    fn get_priority(&self) -> Result<ThreadPriority, Error> {
        get_current_thread_priority()
    }

    /// Sets the current thread's priority.
    fn set_priority(&self, priority: ThreadPriority) -> Result<(), Error> {
        set_current_thread_priority(priority)
    }

    /// Gets the current thread's schedule policy.
    fn get_schedule_policy(&self) -> Result<ThreadSchedulePolicy, Error> {
        thread_schedule_policy()
    }

    /// Returns current thread's schedule policy and parameters.
    fn get_schedule_policy_param(&self) -> Result<(ThreadSchedulePolicy, ScheduleParams), Error> {
        thread_schedule_policy_param(thread_native_id())
    }

    /// Sets current thread's schedule policy and priority.
    fn set_priority_and_policy(
        &self,
        policy: ThreadSchedulePolicy,
        priority: ThreadPriority,
    ) -> Result<(), Error> {
        set_thread_priority_and_policy(thread_native_id(), priority, policy)
    }
}

/// Auto-implementation of this trait for the [`std::thread::Thread`].
impl ThreadExt for std::thread::Thread {
    fn get_native_id(&self) -> Result<ThreadId, Error> {
        if self.id() == std::thread::current().id() {
            Ok(thread_native_id())
        } else {
            Err(Error::Priority(
                "The `ThreadExt::get_native_id()` is currently limited to be called on the current thread.",
            ))
        }
    }
}
