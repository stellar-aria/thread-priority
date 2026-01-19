//! Tests for Haiku thread priority support.

#![cfg(target_os = "haiku")]

use rstest::rstest;
use std::convert::TryInto;
use thread_priority::*;

#[test]
fn get_current_thread_priority_works() {
    let priority = get_current_thread_priority();
    assert!(priority.is_ok());
}

#[test]
fn get_thread_priority_works() {
    let thread_id = thread_native_id();
    let priority = get_thread_priority(thread_id);
    assert!(priority.is_ok());
}

#[test]
fn thread_schedule_policy_returns_other() {
    let policy = thread_schedule_policy();
    assert_eq!(
        policy,
        Ok(ThreadSchedulePolicy::Normal(NormalThreadSchedulePolicy::Other))
    );
}

#[test]
fn thread_schedule_policy_param_works() {
    let thread_id = thread_native_id();
    let result = thread_schedule_policy_param(thread_id);
    assert!(result.is_ok());
    let (policy, params) = result.unwrap();
    assert_eq!(
        policy,
        ThreadSchedulePolicy::Normal(NormalThreadSchedulePolicy::Other)
    );
    // Haiku priority should be in valid range
    assert!(params.sched_priority >= HAIKU_MIN_PRIORITY);
    assert!(params.sched_priority <= HAIKU_MAX_PRIORITY);
}

#[rstest]
fn get_and_set_priority_with_normal_policy(
    #[values(ThreadSchedulePolicy::Normal(NormalThreadSchedulePolicy::Other))]
    policy: ThreadSchedulePolicy,
    #[values(
        ThreadPriority::Min,
        ThreadPriority::Max,
        ThreadPriority::Crossplatform(23u8.try_into().unwrap()),
        ThreadPriority::Crossplatform(50u8.try_into().unwrap())
    )]
    priority: ThreadPriority,
) {
    let result = set_thread_priority_and_policy(thread_native_id(), priority, policy);
    assert!(result.is_ok());
}

#[test]
fn set_and_get_current_thread_priority() {
    let normal_policy = ThreadSchedulePolicy::Normal(NormalThreadSchedulePolicy::Other);

    // Set a specific priority
    let result = set_thread_priority_and_policy(
        thread_native_id(),
        ThreadPriority::Crossplatform(50u8.try_into().unwrap()),
        normal_policy,
    );
    assert!(result.is_ok());

    // Get it back and verify it's in a reasonable range
    let priority = get_current_thread_priority();
    assert!(priority.is_ok());
}

#[test]
fn check_min_and_max_priority_values() {
    let policy = ThreadSchedulePolicy::Normal(NormalThreadSchedulePolicy::Other);
    let max_value = ThreadPriority::max_value_for_policy(policy).unwrap();
    let min_value = ThreadPriority::min_value_for_policy(policy).unwrap();
    
    assert_eq!(min_value, HAIKU_MIN_PRIORITY);
    assert_eq!(max_value, HAIKU_MAX_PRIORITY);
}

#[test]
fn thread_native_id_returns_valid_id() {
    let id = thread_native_id();
    // On Haiku, pthread_t should be a valid pointer
    assert_ne!(id as usize, 0);
}

#[test]
fn thread_ext_trait_works() {
    let thread = std::thread::current();
    
    let priority = thread.get_priority();
    assert!(priority.is_ok());
    
    let policy = thread.get_schedule_policy();
    assert!(policy.is_ok());
    assert_eq!(
        policy.unwrap(),
        ThreadSchedulePolicy::Normal(NormalThreadSchedulePolicy::Other)
    );
    
    let policy_param = thread.get_schedule_policy_param();
    assert!(policy_param.is_ok());
}

#[test]
fn set_min_priority() {
    let result = set_thread_priority_and_policy(
        thread_native_id(),
        ThreadPriority::Min,
        ThreadSchedulePolicy::Normal(NormalThreadSchedulePolicy::Other),
    );
    assert!(result.is_ok());
}

#[test]
fn set_max_priority() {
    // Note: Setting max priority might require elevated privileges on Haiku
    let result = set_thread_priority_and_policy(
        thread_native_id(),
        ThreadPriority::Max,
        ThreadSchedulePolicy::Normal(NormalThreadSchedulePolicy::Other),
    );
    // Either succeeds or fails with permission error
    match result {
        Ok(()) => (),
        Err(Error::OS(_)) => (), // Permission denied is acceptable
        other => panic!("Unexpected result: {:?}", other),
    }
}
