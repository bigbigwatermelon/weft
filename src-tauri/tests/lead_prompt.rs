use atlas_app_lib::lead_chat::commands::lead_prompt;

#[test]
fn lead_prompt_is_generic_agent_app_copy() {
    let prompt = lead_prompt();
    assert!(prompt.contains("local Agent App"));
    assert!(prompt.contains("get_task"));
    assert!(prompt.contains("directly in chat"));
    assert!(!prompt.contains("ask_human"));
    assert!(!prompt.contains("get_repo_map"));
    assert!(!prompt.contains("propose_directions"));
    assert!(!prompt.contains("worktree"));
    assert!(!prompt.contains("PR"));
}
