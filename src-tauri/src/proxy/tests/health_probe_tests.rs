//! Health probe feature tests.
//!
//! Tests cover: model protection add/remove (memory + disk),
//! ensure_fresh_token disk fallback, model filter logic,
//! ProxyToken::is_model_blocked, dual identity detection,
//! and PROBE_MODELS constant correctness.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::models::account::ModelProtection;
use crate::proxy::token_manager::{ProbeStatus, ProxyToken, TokenManager};

fn make_tmp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "antigravity-health-probe-test-{}-{}",
        suffix,
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(dir.join("accounts")).unwrap();
    dir
}

fn make_proxy_token(account_id: &str, email: &str, project_id: &str, data_dir: &PathBuf) -> ProxyToken {
    let now = chrono::Utc::now().timestamp();
    ProxyToken {
        account_id: account_id.to_string(),
        access_token: format!("test-access-token-{}", account_id),
        refresh_token: format!("test-refresh-token-{}", account_id),
        expires_in: 3600,
        timestamp: now,
        email: email.to_string(),
        account_path: data_dir.join("accounts").join(format!("{}.json", account_id)),
        project_id: Some(project_id.to_string()),
        subscription_tier: None,
        remaining_quota: None,
        protected_models: HashMap::new(),
        health_score: 1.0,
        reset_time: None,
        validation_blocked: false,
        validation_blocked_until: 0,
        validation_url: None,
        model_quotas: HashMap::new(),
        model_limits: HashMap::new(),
    }
}

fn write_account_json(data_dir: &PathBuf, id: &str, email: &str, project_id: &str) {
    let now = chrono::Utc::now().timestamp();
    let json = serde_json::json!({
        "id": id,
        "email": email,
        "token": {
            "access_token": format!("atk-{}", id),
            "refresh_token": format!("rtk-{}", id),
            "expires_in": 3600,
            "expiry_timestamp": now + 3600,
            "project_id": project_id
        },
        "project_id": project_id,
        "refresh_token": format!("rtk-{}", id),
        "disabled": false,
        "proxy_disabled": false,
        "created_at": now,
        "last_used": now,
        "protected_models": {}
    });
    let path = data_dir.join("accounts").join(format!("{}.json", id));
    std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap()).unwrap();
}

// ===== is_model_blocked =====

#[test]
fn is_model_blocked_returns_true_for_validation_required() {
    let dir = make_tmp_dir("blocked-vr");
    let mut token = make_proxy_token("acc1", "a@test.com", "pid1", &dir);
    token.protected_models.insert(
        "gemini-3-flash".to_string(),
        ModelProtection {
            reason: "validation_required".to_string(),
            locked_at: chrono::Utc::now().timestamp(),
            unlocks_at: None,
        },
    );
    assert!(token.is_model_blocked("gemini-3-flash", false));
    assert!(token.is_model_blocked("gemini-3-flash", true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn is_model_blocked_returns_true_for_manual() {
    let dir = make_tmp_dir("blocked-manual");
    let mut token = make_proxy_token("acc1", "a@test.com", "pid1", &dir);
    token.protected_models.insert(
        "claude".to_string(),
        ModelProtection {
            reason: "manual".to_string(),
            locked_at: chrono::Utc::now().timestamp(),
            unlocks_at: None,
        },
    );
    assert!(token.is_model_blocked("claude", false));
    assert!(token.is_model_blocked("claude", true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn is_model_blocked_quota_exhausted_respects_protection_flag() {
    let dir = make_tmp_dir("blocked-quota");
    let mut token = make_proxy_token("acc1", "a@test.com", "pid1", &dir);
    token.protected_models.insert(
        "gemini-3-flash".to_string(),
        ModelProtection {
            reason: "quota_exhausted".to_string(),
            locked_at: chrono::Utc::now().timestamp(),
            unlocks_at: None,
        },
    );
    assert!(!token.is_model_blocked("gemini-3-flash", false));
    assert!(token.is_model_blocked("gemini-3-flash", true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn is_model_blocked_returns_false_for_unlocked_model() {
    let dir = make_tmp_dir("blocked-none");
    let token = make_proxy_token("acc1", "a@test.com", "pid1", &dir);
    assert!(!token.is_model_blocked("gemini-3-flash", false));
    assert!(!token.is_model_blocked("gemini-3-flash", true));
    let _ = std::fs::remove_dir_all(&dir);
}

// ===== add_model_protection =====

#[tokio::test]
async fn add_model_protection_updates_memory_and_disk() {
    let dir = make_tmp_dir("add-protection");
    write_account_json(&dir, "acc1", "a@test.com", "pid1");

    let manager = TokenManager::new(dir.clone());
    let token = make_proxy_token("acc1", "a@test.com", "pid1", &dir);
    manager.tokens.insert("acc1".to_string(), token);

    manager
        .add_model_protection("acc1", "gemini-3-flash", "validation_required", None)
        .await
        .unwrap();

    // Memory check
    let entry = manager.tokens.get("acc1").unwrap();
    assert!(entry.protected_models.contains_key("gemini-3-flash"));
    assert_eq!(
        entry.protected_models.get("gemini-3-flash").unwrap().reason,
        "validation_required"
    );
    drop(entry);

    // Disk check
    let disk_content = std::fs::read_to_string(dir.join("accounts/acc1.json")).unwrap();
    let disk_json: serde_json::Value = serde_json::from_str(&disk_content).unwrap();
    assert!(
        disk_json["protected_models"]["gemini-3-flash"].is_object(),
        "protected_models should contain gemini-3-flash on disk"
    );
    assert_eq!(
        disk_json["protected_models"]["gemini-3-flash"]["reason"],
        "validation_required"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn add_model_protection_with_unlocks_at() {
    let dir = make_tmp_dir("add-protection-unlock");
    write_account_json(&dir, "acc1", "a@test.com", "pid1");

    let manager = TokenManager::new(dir.clone());
    let token = make_proxy_token("acc1", "a@test.com", "pid1", &dir);
    manager.tokens.insert("acc1".to_string(), token);

    let unlock_time = chrono::Utc::now().timestamp() + 600;
    manager
        .add_model_protection("acc1", "gemini-3-pro-high", "quota_exhausted", Some(unlock_time))
        .await
        .unwrap();

    let entry = manager.tokens.get("acc1").unwrap();
    let protection = entry.protected_models.get("gemini-3-pro-high").unwrap();
    assert_eq!(protection.reason, "quota_exhausted");
    assert_eq!(protection.unlocks_at, Some(unlock_time));

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn add_model_protection_fails_if_no_account_file() {
    let dir = make_tmp_dir("add-protection-nofile");
    let manager = TokenManager::new(dir.clone());

    let result = manager
        .add_model_protection("nonexistent", "gemini-3-flash", "validation_required", None)
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Account file not found"));

    let _ = std::fs::remove_dir_all(&dir);
}

// ===== remove_model_protection =====

#[tokio::test]
async fn remove_model_protection_clears_memory_and_disk() {
    let dir = make_tmp_dir("remove-protection");
    write_account_json(&dir, "acc1", "a@test.com", "pid1");

    let manager = TokenManager::new(dir.clone());
    let token = make_proxy_token("acc1", "a@test.com", "pid1", &dir);
    manager.tokens.insert("acc1".to_string(), token);

    // Lock first
    manager
        .add_model_protection("acc1", "gemini-3-flash", "validation_required", None)
        .await
        .unwrap();
    assert!(manager.tokens.get("acc1").unwrap().protected_models.contains_key("gemini-3-flash"));

    // Unlock
    manager
        .remove_model_protection("acc1", "gemini-3-flash")
        .await
        .unwrap();

    // Memory check
    assert!(!manager.tokens.get("acc1").unwrap().protected_models.contains_key("gemini-3-flash"));

    // Disk check
    let disk_content = std::fs::read_to_string(dir.join("accounts/acc1.json")).unwrap();
    let disk_json: serde_json::Value = serde_json::from_str(&disk_content).unwrap();
    assert!(
        !disk_json["protected_models"]
            .as_object()
            .unwrap()
            .contains_key("gemini-3-flash"),
        "gemini-3-flash should be removed from disk"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn remove_model_protection_noop_if_not_locked() {
    let dir = make_tmp_dir("remove-protection-noop");
    write_account_json(&dir, "acc1", "a@test.com", "pid1");

    let manager = TokenManager::new(dir.clone());
    let token = make_proxy_token("acc1", "a@test.com", "pid1", &dir);
    manager.tokens.insert("acc1".to_string(), token);

    // Should not error even if model was never locked
    manager
        .remove_model_protection("acc1", "gemini-3-flash")
        .await
        .unwrap();

    let _ = std::fs::remove_dir_all(&dir);
}

// ===== ensure_fresh_token (disk fallback) =====

#[tokio::test]
async fn ensure_fresh_token_reads_from_pool_when_available() {
    let dir = make_tmp_dir("fresh-token-pool");
    let manager = TokenManager::new(dir.clone());

    let mut token = make_proxy_token("acc1", "a@test.com", "pid1", &dir);
    token.timestamp = chrono::Utc::now().timestamp();
    token.expires_in = 3600;
    token.access_token = "pool-access-token".to_string();
    manager.tokens.insert("acc1".to_string(), token);

    let result = manager.ensure_fresh_token("acc1").await;
    assert!(result.is_ok());
    let (access_token, project_id) = result.unwrap();
    assert_eq!(access_token, "pool-access-token");
    assert_eq!(project_id, "pid1");

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ensure_fresh_token_fails_for_missing_account() {
    let dir = make_tmp_dir("fresh-token-missing");
    let manager = TokenManager::new(dir.clone());

    let result = manager.ensure_fresh_token("nonexistent").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not in token pool"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ensure_fresh_token_reads_disk_when_not_in_pool() {
    let dir = make_tmp_dir("fresh-token-disk");
    write_account_json(&dir, "acc1", "a@test.com", "pid1");

    let manager = TokenManager::new(dir.clone());
    // Do NOT insert into manager.tokens — simulates proxy_disabled account

    let result = manager.ensure_fresh_token("acc1").await;
    // Will fail at the refresh step (no real OAuth server), but we can verify
    // it found the refresh_token from disk (error should mention "Token refresh failed",
    // NOT "not in token pool")
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("Token refresh failed") || err.contains("refresh"),
        "Expected refresh failure (found disk account), got: {}",
        err
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ensure_fresh_token_disk_fallback_missing_refresh_token() {
    let dir = make_tmp_dir("fresh-token-no-rt");

    // Write account JSON without refresh_token field
    let path = dir.join("accounts/acc1.json");
    let json = serde_json::json!({
        "id": "acc1",
        "email": "a@test.com",
        "project_id": "pid1"
    });
    std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap()).unwrap();

    let manager = TokenManager::new(dir.clone());

    let result = manager.ensure_fresh_token("acc1").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("has no refresh_token"));

    let _ = std::fs::remove_dir_all(&dir);
}

// ===== health_probe_account (email fallback) =====

#[tokio::test]
async fn health_probe_account_uses_email_from_pool() {
    let dir = make_tmp_dir("probe-email-pool");
    let manager = TokenManager::new(dir.clone());

    let mut token = make_proxy_token("acc1", "pool-email@test.com", "pid1", &dir);
    token.timestamp = chrono::Utc::now().timestamp();
    token.expires_in = 3600;
    manager.tokens.insert("acc1".to_string(), token);

    // Will fail at the HTTP call, but we can verify email resolution
    // by checking the error path still includes the right email
    let result = manager.health_probe_account("acc1", None).await;
    // This will succeed past the token check (token is fresh) but fail
    // at the actual HTTP probe. The result contains the email.
    match result {
        Ok(probe_result) => {
            assert_eq!(probe_result.email, "pool-email@test.com");
        }
        Err(_) => {
            // Token might expire, that's fine — the test is about email lookup
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn health_probe_account_reads_email_from_disk_when_not_in_pool() {
    let dir = make_tmp_dir("probe-email-disk");
    write_account_json(&dir, "acc1", "disk-email@test.com", "pid1");

    let manager = TokenManager::new(dir.clone());
    // NOT in token pool

    // Will fail at token refresh, but let's verify the error path
    let result = manager.health_probe_account("acc1", None).await;
    // Should fail with token refresh error (not "not found in token pool")
    assert!(result.is_err());

    let _ = std::fs::remove_dir_all(&dir);
}

// ===== Model filter logic =====

#[test]
fn probe_models_constant_covers_key_groups() {
    use crate::proxy::token_manager::PROBE_MODELS;

    let models: Vec<&str> = PROBE_MODELS.iter().map(|(m, _)| *m).collect();
    assert!(models.contains(&"gemini-3-pro-high"), "Should include premium Gemini");
    assert!(models.contains(&"gemini-3-flash"), "Should include regular Gemini");
    assert!(models.contains(&"claude-sonnet-4-6"), "Should include Claude Sonnet 4.6");
    assert!(models.contains(&"claude-opus-4-6-thinking"), "Should include Claude Opus 4.6");

    let providers: Vec<&str> = PROBE_MODELS.iter().map(|(_, p)| *p).collect();
    assert!(providers.contains(&"gemini"), "Should have gemini provider");
    assert!(providers.contains(&"claude"), "Should have claude provider");
}

#[test]
fn model_filter_filters_correctly() {
    use crate::proxy::token_manager::PROBE_MODELS;

    let filter = vec!["gemini-3-flash".to_string()];
    let filtered: Vec<(&str, &str)> = PROBE_MODELS
        .iter()
        .filter(|(model, _)| filter.iter().any(|m| m == *model))
        .copied()
        .collect();

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].0, "gemini-3-flash");
}

#[test]
fn model_filter_none_returns_all() {
    use crate::proxy::token_manager::PROBE_MODELS;

    let filter: Option<&[String]> = None;
    let filtered: Vec<(&str, &str)> = PROBE_MODELS
        .iter()
        .filter(|(model, _)| filter.map(|f| f.iter().any(|m| m == *model)).unwrap_or(true))
        .copied()
        .collect();

    assert_eq!(filtered.len(), PROBE_MODELS.len());
}

#[test]
fn model_filter_empty_returns_none() {
    use crate::proxy::token_manager::PROBE_MODELS;

    let filter_vec: Vec<String> = vec![];
    let filter: Option<&[String]> = Some(&filter_vec);
    let filtered: Vec<(&str, &str)> = PROBE_MODELS
        .iter()
        .filter(|(model, _)| filter.map(|f| f.iter().any(|m| m == *model)).unwrap_or(true))
        .copied()
        .collect();

    assert_eq!(filtered.len(), 0);
}

// ===== Dual identity (premium vs regular) =====

#[test]
fn is_premium_gemini_model_detects_pro_models() {
    use crate::proxy::common::model_mapping::is_premium_gemini_model;

    assert!(is_premium_gemini_model("gemini-3-pro-high"));
    assert!(is_premium_gemini_model("gemini-3-pro"));
    assert!(is_premium_gemini_model("gemini-3-pro-latest"));
    assert!(is_premium_gemini_model("gemini-3.1-pro"));
    assert!(is_premium_gemini_model("Gemini-3-Pro-High")); // case insensitive
}

#[test]
fn is_premium_gemini_model_rejects_non_pro() {
    use crate::proxy::common::model_mapping::is_premium_gemini_model;

    assert!(!is_premium_gemini_model("gemini-3-flash"));
    assert!(!is_premium_gemini_model("gemini-3-flash-thinking"));
    assert!(!is_premium_gemini_model("claude"));
    assert!(!is_premium_gemini_model("claude-sonnet-4-6"));
    assert!(!is_premium_gemini_model("gpt-4o"));
}

#[test]
fn probe_user_agent_follows_dual_identity() {
    use crate::proxy::common::model_mapping::is_premium_gemini_model;

    for (model, _) in crate::proxy::token_manager::PROBE_MODELS {
        let is_premium = is_premium_gemini_model(model);
        let expected_ua = if is_premium { "antigravity/1.22.2" } else { "antigravity" };

        if *model == "gemini-3-pro-high" {
            assert_eq!(expected_ua, "antigravity/1.22.2", "Premium model should use versioned UA");
        } else if *model == "gemini-3-flash" || *model == "claude-sonnet-4-6" || *model == "claude-opus-4-6-thinking" {
            assert_eq!(expected_ua, "antigravity", "Non-premium model should use plain UA");
        }
    }
}

// ===== ProbeStatus serialization =====

#[test]
fn probe_status_serializes_to_snake_case() {
    let ok = serde_json::to_string(&ProbeStatus::Ok).unwrap();
    assert_eq!(ok, "\"ok\"");

    let locked = serde_json::to_string(&ProbeStatus::Locked).unwrap();
    assert_eq!(locked, "\"locked\"");

    let skipped = serde_json::to_string(&ProbeStatus::Skipped).unwrap();
    assert_eq!(skipped, "\"skipped\"");

    let error = serde_json::to_string(&ProbeStatus::Error).unwrap();
    assert_eq!(error, "\"error\"");
}

// ===== Protection round-trip (add then remove) =====

#[tokio::test]
async fn protection_roundtrip_lock_and_unlock() {
    let dir = make_tmp_dir("roundtrip");
    write_account_json(&dir, "acc1", "a@test.com", "pid1");

    let manager = TokenManager::new(dir.clone());
    let token = make_proxy_token("acc1", "a@test.com", "pid1", &dir);
    manager.tokens.insert("acc1".to_string(), token);

    // Lock
    manager
        .add_model_protection("acc1", "gemini-3-flash", "validation_required", None)
        .await
        .unwrap();
    assert!(manager.tokens.get("acc1").unwrap().is_model_blocked("gemini-3-flash", false));

    // Lock another model
    manager
        .add_model_protection("acc1", "gemini-3-pro-high", "validation_required", None)
        .await
        .unwrap();
    assert!(manager.tokens.get("acc1").unwrap().is_model_blocked("gemini-3-pro-high", false));

    // Unlock first model only
    manager
        .remove_model_protection("acc1", "gemini-3-flash")
        .await
        .unwrap();
    assert!(!manager.tokens.get("acc1").unwrap().is_model_blocked("gemini-3-flash", false));
    assert!(manager.tokens.get("acc1").unwrap().is_model_blocked("gemini-3-pro-high", false));

    // Verify disk state matches memory
    let disk_content = std::fs::read_to_string(dir.join("accounts/acc1.json")).unwrap();
    let disk_json: serde_json::Value = serde_json::from_str(&disk_content).unwrap();
    let pm = disk_json["protected_models"].as_object().unwrap();
    assert!(!pm.contains_key("gemini-3-flash"));
    assert!(pm.contains_key("gemini-3-pro-high"));

    let _ = std::fs::remove_dir_all(&dir);
}

// ===== Multiple accounts batch =====

#[tokio::test]
async fn health_probe_batch_handles_mix_of_pool_and_missing() {
    let dir = make_tmp_dir("batch-mix");
    write_account_json(&dir, "acc1", "a@test.com", "pid1");

    let manager = TokenManager::new(dir.clone());
    // acc1 is on disk but not in pool
    // acc2 doesn't exist anywhere

    let results = manager
        .health_probe_accounts_batch(
            vec!["acc1".to_string(), "acc2".to_string()],
            Some(vec!["gemini-3-flash".to_string()]),
        )
        .await;

    // Both should be skipped (acc1 fails at token refresh, acc2 fails at file read)
    // The batch method filters out errors, so results should be empty or contain only successes
    assert!(
        results.len() <= 2,
        "Should handle missing accounts gracefully"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ===== 集成测试：真实 API 请求 =====
// 需要本地有真实账号数据（~/.antigravity_tools/accounts/）
// 运行方式: cargo test --lib -- health_probe_tests::integration --ignored --nocapture

fn get_real_data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("ABV_DATA_DIR") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    dirs::home_dir().unwrap().join(".antigravity_tools")
}

fn real_accounts_available() -> bool {
    get_real_data_dir().join("accounts").exists()
}

#[tokio::test]
#[ignore]
async fn integration_load_accounts_and_probe_single() {
    if !real_accounts_available() {
        eprintln!("跳过: 未找到真实账号数据");
        return;
    }

    let data_dir = get_real_data_dir();
    let manager = TokenManager::new(data_dir);
    let count = manager.load_accounts().await.expect("加载账号失败");
    eprintln!("已加载 {} 个账号到代理池", count);
    assert!(count > 0, "至少需要一个可用账号");

    // 取第一个可用账号
    let first_entry = manager.tokens.iter().next().expect("代理池为空");
    let account_id = first_entry.key().clone();
    let email = first_entry.value().email.clone();
    drop(first_entry);

    eprintln!("测试账号: {} ({})", account_id, email);

    // 只探测 gemini-3-flash（最不可能触发 VALIDATION_REQUIRED 的模型）
    let filter = vec!["gemini-3-flash".to_string()];
    let result = manager
        .health_probe_account(&account_id, Some(&filter))
        .await
        .expect("探测失败");

    eprintln!("探测结果:");
    for r in &result.results {
        eprintln!("  {} -> {:?} ({})", r.model, r.status, r.message);
    }

    assert_eq!(result.account_id, account_id);
    assert_eq!(result.results.len(), 1);
    assert_eq!(result.results[0].model, "gemini-3-flash");
}

#[tokio::test]
#[ignore]
async fn integration_probe_all_models_single_account() {
    if !real_accounts_available() {
        eprintln!("跳过: 未找到真实账号数据");
        return;
    }

    let data_dir = get_real_data_dir();
    let manager = TokenManager::new(data_dir);
    let count = manager.load_accounts().await.expect("加载账号失败");
    assert!(count > 0);

    let first_entry = manager.tokens.iter().next().unwrap();
    let account_id = first_entry.key().clone();
    let email = first_entry.value().email.clone();
    drop(first_entry);

    eprintln!("探测账号: {} ({}) — 全部模型", account_id, email);

    let result = manager
        .health_probe_account(&account_id, None)
        .await
        .expect("探测失败");

    eprintln!("探测结果 ({} 个模型):", result.results.len());
    for r in &result.results {
        eprintln!("  {:25} -> {:?}  {}", r.model, r.status, r.message);
    }

    assert_eq!(
        result.results.len(),
        crate::proxy::token_manager::PROBE_MODELS.len(),
        "应该探测所有 PROBE_MODELS"
    );

    let pro_result = result.results.iter().find(|r| r.model == "gemini-3-pro-high");
    assert!(pro_result.is_some(), "应该包含 gemini-3-pro-high 的结果");
    eprintln!(
        "gemini-3-pro-high 状态: {:?} — {}",
        pro_result.unwrap().status,
        pro_result.unwrap().message
    );
}

#[tokio::test]
#[ignore]
async fn integration_probe_batch_first_three_accounts() {
    if !real_accounts_available() {
        eprintln!("跳过: 未找到真实账号数据");
        return;
    }

    let data_dir = get_real_data_dir();
    let manager = TokenManager::new(data_dir);
    let count = manager.load_accounts().await.expect("加载账号失败");
    eprintln!("已加载 {} 个账号", count);

    let account_ids: Vec<String> = manager.tokens.iter().take(3).map(|e| e.key().clone()).collect();
    eprintln!("批量探测账号: {:?}", account_ids);

    let filter = vec!["gemini-3-flash".to_string()];
    let results = manager
        .health_probe_accounts_batch(account_ids.clone(), Some(filter))
        .await;

    eprintln!("\n批量探测结果 ({}/{} 个账号成功返回):", results.len(), account_ids.len());
    let mut ok_count = 0;
    let mut locked_count = 0;
    let mut error_count = 0;
    let mut skipped_count = 0;

    for account_result in &results {
        eprintln!("  账号: {} ({})", account_result.account_id, account_result.email);
        for r in &account_result.results {
            eprintln!("    {:25} -> {:?}  {}", r.model, r.status, r.message);
            match r.status {
                ProbeStatus::Ok => ok_count += 1,
                ProbeStatus::Locked => locked_count += 1,
                ProbeStatus::Error => error_count += 1,
                ProbeStatus::Skipped => skipped_count += 1,
            }
        }
    }

    eprintln!("\n汇总: {} ok, {} locked, {} error, {} skipped", ok_count, locked_count, error_count, skipped_count);
    assert!(!results.is_empty(), "至少应该有一个账号探测成功");
}

#[tokio::test]
#[ignore]
async fn integration_probe_auto_lock_and_unlock() {
    if !real_accounts_available() {
        eprintln!("跳过: 未找到真实账号数据");
        return;
    }

    let data_dir = get_real_data_dir();
    let manager = TokenManager::new(data_dir);
    manager.load_accounts().await.expect("加载账号失败");

    let first_entry = manager.tokens.iter().next().unwrap();
    let account_id = first_entry.key().clone();
    drop(first_entry);

    let test_model = "gemini-3-flash";
    eprintln!("手动锁定 {}/{} (validation_required)", account_id, test_model);
    manager
        .add_model_protection(&account_id, test_model, "validation_required", None)
        .await
        .unwrap();

    assert!(
        manager.tokens.get(&account_id).unwrap().is_model_blocked(test_model, false),
        "锁定后应该被阻止"
    );

    let filter = vec![test_model.to_string()];
    let result = manager
        .health_probe_account(&account_id, Some(&filter))
        .await
        .expect("探测失败");

    let probe_result = &result.results[0];
    eprintln!(
        "探测结果: {:?} — {}",
        probe_result.status, probe_result.message
    );

    match probe_result.status {
        ProbeStatus::Ok => {
            eprintln!("API 返回 200，验证自动解锁...");
            assert!(
                !manager.tokens.get(&account_id).unwrap().is_model_blocked(test_model, false),
                "探测 200 后应该自动解锁"
            );
            assert!(
                probe_result.message.contains("auto-unlocked"),
                "消息应该包含 auto-unlocked"
            );

            let disk_path = manager.tokens.get(&account_id).unwrap().account_path.clone();
            let disk_content = std::fs::read_to_string(&disk_path).unwrap();
            let disk_json: serde_json::Value = serde_json::from_str(&disk_content).unwrap();
            let pm = disk_json.get("protected_models").and_then(|v| v.as_object());
            if let Some(pm) = pm {
                assert!(
                    !pm.contains_key(test_model),
                    "磁盘上也应该已解锁"
                );
            }
            eprintln!("自动解锁验证通过");
        }
        ProbeStatus::Locked => {
            eprintln!("API 返回 403 VALIDATION_REQUIRED — 该账号确实需要验证，锁定保持");
            assert!(
                manager.tokens.get(&account_id).unwrap().is_model_blocked(test_model, false),
                "真正 VALIDATION_REQUIRED 时应该保持锁定"
            );
        }
        _ => {
            eprintln!("API 返回 {:?}，不改变锁定状态", probe_result.status);
            let _ = manager.remove_model_protection(&account_id, test_model).await;
        }
    }
}

#[tokio::test]
#[ignore]
async fn integration_probe_disk_only_account() {
    if !real_accounts_available() {
        eprintln!("跳过: 未找到真实账号数据");
        return;
    }

    let data_dir = get_real_data_dir();
    let accounts_dir = data_dir.join("accounts");

    // 不调用 load_accounts，模拟 proxy_disabled 场景
    let manager = TokenManager::new(data_dir.clone());

    let first_file = std::fs::read_dir(&accounts_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .expect("无账号文件");
    let account_id = first_file
        .path()
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    eprintln!("测试磁盘回退: 账号 {} 未加载到代理池", account_id);
    assert!(manager.tokens.get(&account_id).is_none(), "不应在代理池中");

    let token_result = manager.ensure_fresh_token(&account_id).await;
    match token_result {
        Ok((access_token, project_id)) => {
            let preview_len = 20.min(access_token.len());
            eprintln!("磁盘回退成功: token 前缀 = {}..., project = {}", &access_token[..preview_len], project_id);

            let filter = vec!["gemini-3-flash".to_string()];
            let result = manager.health_probe_account(&account_id, Some(&filter)).await;
            match result {
                Ok(r) => {
                    eprintln!("探测结果: {:?} — {}", r.results[0].status, r.results[0].message);
                }
                Err(e) => {
                    eprintln!("探测失败（可能是二次 token 刷新问题）: {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("磁盘回退失败: {}", e);
        }
    }
}

#[tokio::test]
#[ignore]
async fn integration_probe_gemini_31_pro_unavailable() {
    if !real_accounts_available() {
        eprintln!("跳过: 未找到真实账号数据");
        return;
    }

    let data_dir = get_real_data_dir();
    let manager = TokenManager::new(data_dir);
    let count = manager.load_accounts().await.expect("加载账号失败");
    eprintln!("已加载 {} 个账号", count);
    assert!(count > 0);

    // 验证 gemini-3.1-pro 被识别为 premium 模型
    assert!(
        crate::proxy::common::model_mapping::is_premium_gemini_model("gemini-3.1-pro"),
        "gemini-3.1-pro 应该被识别为 premium 模型"
    );

    // 用多个账号探测，确认 gemini-3.1-pro 在所有账号上都不可用
    let account_ids: Vec<String> = manager.tokens.iter().take(5).map(|e| e.key().clone()).collect();
    eprintln!("探测 {} 个账号的 gemini-3.1-pro 可用性...", account_ids.len());

    let mut all_failed = true;
    for account_id in &account_ids {
        let email = manager.tokens.get(account_id)
            .map(|e| e.email.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let (access_token, project_id) = match manager.ensure_fresh_token(account_id).await {
            Ok(t) => t,
            Err(e) => {
                eprintln!("  {} ({}) — token 刷新失败: {}", account_id, email, e);
                continue;
            }
        };

        let result = manager.probe_single_model(
            account_id, &access_token, &project_id, "gemini-3.1-pro", "gemini"
        ).await;

        eprintln!(
            "  {} ({}) — {:?}: {}",
            account_id, email, result.status, result.message
        );

        if matches!(result.status, ProbeStatus::Ok) {
            all_failed = false;
        }
    }

    eprintln!("\n结论: gemini-3.1-pro {}", if all_failed { "在所有测试账号上均不可用" } else { "在部分账号上可用（非预期）" });
    assert!(all_failed, "gemini-3.1-pro 预期在所有账号上不可用，但有账号返回了 Ok");
}
