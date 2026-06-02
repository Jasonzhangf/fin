use super::*;

#[test]
fn headless_daemon_startup_supervises_builtin_qqbot_bridge_when_credentials_present() {
    let _guard = env_lock().lock().expect("env lock");
    let previous_cycles = std::env::var("FIN_HEADLESS_DAEMON_MAX_CYCLES").ok();
    let previous_path = std::env::var("PATH").ok();
    unsafe {
        std::env::set_var("FIN_HEADLESS_DAEMON_MAX_CYCLES", "1");
    }

    let home = temp_runtime_home("qqbot-bridge");
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    fs::create_dir_all(home.join("config")).expect("config dir");
    let user_toml = sample_user_toml_with_qqbot();
    fs::write(home.join("config/user.toml"), user_toml.as_bytes()).expect("user toml");
    let fake_node = install_fake_node(&home);
    let fake_path = fake_node
        .parent()
        .expect("fake node parent")
        .display()
        .to_string();
    let merged_path = match previous_path.as_deref() {
        Some(existing) if !existing.trim().is_empty() => format!("{fake_path}:{existing}"),
        _ => fake_path,
    };
    unsafe {
        std::env::set_var("PATH", merged_path);
    }

    let system = map_system_config(&user_toml).expect("system config");
    let report = run_headless_daemon_with_provider(
        &user_toml,
        &system,
        &HeadlessResumeProvider::new(&system),
        home.clone(),
    )
    .expect("headless daemon run");

    assert_eq!(report.cycles_completed, 1);
    let peer_events =
        fs::read_to_string(home.join("runtime/peers/qqbot/events.jsonl")).expect("peer events");
    assert!(peer_events.contains("channel.peer.bridge_spawned"));
    assert!(peer_events.contains("channel.peer.bridge_start_requested"));
    assert!(peer_events.contains("channel.peer.bridge_process_exited"));
    assert!(!peer_events.contains("channel.peer.bridge_start_skipped"));
    assert!(
        home.join("runtime/peers/qqbot/bin/qqbot-peer-runner.mjs")
            .exists()
    );

    if let Some(value) = previous_cycles {
        unsafe {
            std::env::set_var("FIN_HEADLESS_DAEMON_MAX_CYCLES", value);
        }
    } else {
        unsafe {
            std::env::remove_var("FIN_HEADLESS_DAEMON_MAX_CYCLES");
        }
    }
    if let Some(path) = previous_path {
        unsafe {
            std::env::set_var("PATH", path);
        }
    } else {
        unsafe {
            std::env::remove_var("PATH");
        }
    }
}

#[test]
fn stop_headless_daemon_writes_stop_request() {
    let home = temp_runtime_home("stop");
    ensure_runtime_home_layout(&home).expect("runtime home");
    let user_toml = sample_user_toml();
    let system = map_system_config(&user_toml).expect("system config");
    fs::create_dir_all(home.join("runtime/pids")).expect("pid dir");
    fs::write(home.join("runtime/pids/headless-daemon.pid"), b"12345").expect("pid");

    let report =
        stop_headless_daemon(&user_toml, &system, Some(home.as_path())).expect("stop report");
    assert_eq!(report.status, "stop_requested");
    assert_eq!(report.pid, Some(12345));
    assert!(home.join("runtime/locks/headless-daemon.stop").exists());
}
