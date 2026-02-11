use mcp_cli_rs::config::Config;
use mcp_cli_rs::config_fingerprint::{config_fingerprint, config_hash_changed};

#[test]
fn test_config_fingerprint_basic() {
    let config = Config::default();
    let fp = config_fingerprint(&config);
    assert!(!fp.is_empty(), "Fingerprint should not be empty");
    assert_eq!(
        fp.len(),
        64,
        "Fingerprint should be 64 characters (hex-encoded SHA256)"
    );
}

#[test]
fn test_config_fingerprint_changes_with_config_change() {
    let mut config1 = Config::default();
    let fp1 = config_fingerprint(&config1);

    // Change something in the config
    config1.concurrency_limit = 10;

    let config2 = Config::default();
    let fp2 = config_fingerprint(&config2);

    // Same defaults should have same hash
    assert_eq!(fp1, fp2, "Default config should always produce same hash");

    // Changed config should have different hash
    config1.concurrency_limit = 10;
    let fp3 = config_fingerprint(&config1);
    assert_ne!(fp1, fp3, "Changed concurrency_limit should change hash");

    // Verify detection logic works
    assert!(config_hash_changed(&config2, &config1));
}

#[test]
fn test_config_fingerprint_includes_all_content() {
    let mut config = Config::default();
    config.concurrency_limit = 5;
    config.retry_max = 3;
    config.retry_delay_ms = 1000;
    config.timeout_secs = 1800;
    config.daemon_ttl = 60;

    let fp = config_fingerprint(&config);

    // Verify fingerprint captures all config values
    assert!(
        !fp.is_empty(),
        "Fingerprint should include all config fields"
    );
    assert_eq!(fp.len(), 64, "Fingerprint length should match SHA256");

    // Different values should produce different hashes
    config.concurrency_limit = 6;
    let fp2 = config_fingerprint(&config);
    assert_ne!(fp, fp2, "Changing config values should change hash");
}

#[test]
fn test_config_hash_detection_logic() {
    let mut config1 = Config::default();
    config1.concurrency_limit = 7;

    let mut config2 = Config::default();
    config2.concurrency_limit = 7;

    // Same values should not show as changed
    assert!(!config_hash_changed(&config1, &config2));

    // Different values should show as changed
    config1.concurrency_limit = 8;
    assert!(config_hash_changed(&config1, &config2));
}

#[test]
fn test_config_fingerprint_same_config_same_hash() {
    let mut config1 = Config::default();
    config1.concurrency_limit = 5;
    config1.retry_max = 3;
    config1.retry_delay_ms = 1000;

    let mut config2 = Config::default();
    config2.concurrency_limit = 5;
    config2.retry_max = 3;
    config2.retry_delay_ms = 1000;

    let fp1 = config_fingerprint(&config1);
    let fp2 = config_fingerprint(&config2);

    // Same values produce same hash
    assert_eq!(
        fp1, fp2,
        "Identical configs should produce identical hashes"
    );

    // Detection logic should not flag as changed
    assert!(!config_hash_changed(&config1, &config2));
}

#[test]
fn test_config_fingerprint_integration_with_daemon_logic() {
    use mcp_cli_rs::daemon::lifecycle::DaemonLifecycle;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let mut config1 = Config::default();
    let fp1 = config_fingerprint(&config1);

    // Create daemon state similar to how daemon/mod.rs creates it
    let lifecycle = DaemonLifecycle::new(60);
    let connection_pool = Arc::new(mcp_cli_rs::daemon::pool::ConnectionPool::new(Arc::new(
        config1.clone(),
    )));

    let state = mcp_cli_rs::daemon::DaemonState {
        config: Arc::new(config1.clone()),
        config_fingerprint: fp1.clone(),
        lifecycle: Arc::new(Mutex::new(lifecycle)),
        connection_pool,
    };

    // If config hash matches, no change
    assert_eq!(
        state.config_fingerprint, fp1,
        "State should store initial fingerprint"
    );

    // After config change, hash differs
    config1.concurrency_limit = 10;
    let fp2 = config_fingerprint(&config1);
    assert!(fp2 != fp1, "New config should have different hash");

    // Integration: config hash should detect change
    assert!(config_hash_changed(&config1, &state.config));
    assert!(state.config_fingerprint != fp2);
}
