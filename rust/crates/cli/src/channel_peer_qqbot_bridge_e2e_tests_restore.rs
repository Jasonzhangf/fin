use super::*;

#[test]
fn qqbot_inbound_restores_existing_target_session_and_continues_same_session() {
    let first_response = r#"{"id":"msg-qqbot-2","content":[{"type":"text","text":"<fin_user_response>第一次回复。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"task_completed\":true,\"is_simple_chat\":false,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":\"task-restore\",\"candidate_topic_thread_id\":\"topic-restore\",\"continuity_confidence\":90,\"topic_shift_confidence\":10,\"simple_query_confidence\":5,\"previous_topic_summary\":\"restore\",\"current_topic_summary\":\"restore\",\"completion_evidence\":[\"first inbound reply generated\",\"target binding persisted\"],\"final_conclusions\":[\"first qqbot closure completed\",\"target session remains active\"],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":\"first reply\",\"digest_candidate\":\"first reply\",\"reason\":\"first turn\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"first turn done\"}}]</fin_tool_calls>"}],"stop_reason":"end_turn"}"#;
    let second_response = r#"{"id":"msg-qqbot-3","content":[{"type":"text","text":"<fin_user_response>第二次回复，沿用旧会话。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"task_completed\":true,\"is_simple_chat\":false,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":\"task-restore\",\"candidate_topic_thread_id\":\"topic-restore\",\"continuity_confidence\":94,\"topic_shift_confidence\":6,\"simple_query_confidence\":4,\"previous_topic_summary\":\"restore\",\"current_topic_summary\":\"restore\",\"completion_evidence\":[\"existing target session restored\",\"second inbound reply persisted\"],\"final_conclusions\":[\"restore closure completed\",\"existing target binding wins\"],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":\"restored existing target session\",\"digest_candidate\":\"restored existing target session\",\"reason\":\"existing target binding wins\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"second turn done\"}}]</fin_tool_calls>"}],"stop_reason":"end_turn"}"#;
    let (base_url, requests, server) =
        spawn_anthropic_server(vec![first_response, second_response]);
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home");
    let user_toml = sample_user_toml(&base_url);
    let system = map_system_config(&user_toml).expect("system config");
    let handler = CliDebugActionHandler::new(user_toml, system).expect("handler");
    let (session_a, task_a) = new_session(&handler, &home).expect("session a");
    complete_builtin_qqbot_pairing(&home, &session_a, None).expect("pair session a");

    let sink_path = home.join("runtime/peers/qqbot/outbound-restore.jsonl");
    let (child, stdin) = spawn_bridge_sink(&sink_path);
    process_inbound_message(
        &home,
        &handler,
        &stdin,
        direct_message("user-restore", "msg-restore-1", "第一条消息，创建目标绑定"),
    )
    .expect("first inbound");

    let (session_b, _) = new_distinct_session(&handler, &home, &session_a).expect("session b");
    complete_builtin_qqbot_pairing(&home, &session_b, None).expect("pair session b");

    process_inbound_message(
        &home,
        &handler,
        &stdin,
        direct_message("user-restore", "msg-restore-2", "第二条消息，应恢复旧会话"),
    )
    .expect("second inbound");
    let outbound = finish_bridge_sink(&stdin, child, &sink_path);
    server.join().expect("server join");

    assert_eq!(outbound.len(), 4, "two ack + two replies");
    let restored_reply = outbound[3]["payload"]["text"]
        .as_str()
        .expect("restored reply");
    assert!(restored_reply.contains("🤖 System Agent"));
    assert!(restored_reply.contains("第二次回复，沿用旧会话。"));

    let conversation = load_conversation_by_target(&home, "qqbot:c2c:user-restore")
        .expect("conversation")
        .expect("present");
    assert_eq!(conversation.session_id.as_deref(), Some(session_a.as_str()));
    assert_eq!(
        conversation.last_inbound_message_id.as_deref(),
        Some("msg-restore-2")
    );

    let last_run = read_last_run_value(&home).expect("last run");
    assert_eq!(last_run["session_id"].as_str(), Some(session_a.as_str()));
    assert_eq!(last_run["task_id"].as_str(), Some(task_a.as_str()));

    let (_, _, session_a_dir) = find_session_dir(&home, &session_a).expect("session a dir");
    let session_a_messages =
        read_session_messages(&session_a_dir.join("conversation/messages.json"))
            .expect("session a messages");
    assert!(
        session_a_messages
            .iter()
            .filter(|message| message.role == "user")
            .any(|message| message.content.contains("第一条消息"))
    );
    assert!(
        session_a_messages
            .iter()
            .filter(|message| message.role == "user")
            .any(|message| message.content.contains("第二条消息"))
    );

    let (_, _, session_b_dir) = find_session_dir(&home, &session_b).expect("session b dir");
    let session_b_messages =
        read_session_messages(&session_b_dir.join("conversation/messages.json"))
            .expect("session b messages");
    assert!(
        session_b_messages
            .iter()
            .all(|message| !message.content.contains("第二条消息"))
    );

    let peer_events =
        fs::read_to_string(home.join("runtime/peers/qqbot/events.jsonl")).expect("peer events");
    assert!(peer_events.contains("channel.peer.session_restored"));
    assert!(peer_events.contains("msg-restore-2"));

    let captured_requests = requests.lock().expect("requests").clone();
    assert_eq!(captured_requests.len(), 2);
    assert!(captured_requests[1].contains("第一条消息，创建目标绑定"));
    assert!(captured_requests[1].contains("第二条消息，应恢复旧会话"));
}

#[test]
fn qqbot_inbound_forwards_formalize_system_notice_instead_of_no_new_messages() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home");
    let user_toml = sample_user_toml("http://127.0.0.1:9");
    let system = map_system_config(&user_toml).expect("system config");
    let handler = CliDebugActionHandler::new(user_toml, system).expect("handler");
    let (session_id, _) = new_session(&handler, &home).expect("new session");
    complete_builtin_qqbot_pairing(&home, &session_id, None).expect("pair session");

    let sink_path = home.join("runtime/peers/qqbot/outbound-routing-prompt.jsonl");
    let (child, stdin) = spawn_bridge_sink(&sink_path);
    process_inbound_message(
        &home,
        &handler,
        &stdin,
        direct_message("user-routing", "msg-routing-1", "/formalize"),
    )
    .expect("process inbound");
    let outbound = finish_bridge_sink(&stdin, child, &sink_path);

    assert_eq!(outbound.len(), 2, "ack + instant local-command notice");
    let ack_text = outbound[0]["payload"]["text"].as_str().expect("ack text");
    assert!(ack_text.contains("📡 QQ Channel Peer"));
    assert!(ack_text.contains("已收到，正在处理。"));
    let notice_text = outbound[1]["payload"]["text"]
        .as_str()
        .expect("formalize notice text");
    assert!(
        notice_text.contains("formalize skipped")
            || notice_text.contains("already bound to a task")
    );
    assert!(!notice_text.contains("没有生成新的可发送回复"));

    let peer_events =
        fs::read_to_string(home.join("runtime/peers/qqbot/events.jsonl")).expect("peer events");
    assert!(!peer_events.contains("channel.peer.delivery_pending_no_new_messages"));
    assert!(!peer_events.contains("\"notice_kind\":\"no_new_messages\""));
}
