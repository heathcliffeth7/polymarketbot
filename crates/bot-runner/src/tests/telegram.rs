use super::support::*;
use super::*;

#[test]
fn telegram_chat_id_prefers_node_override() {
    let telegram = TelegramConfig {
        bot_token: "settings-token".to_string(),
        chat_id: "-100settings".to_string(),
    };
    let node = telegram_node(json!({
        "chatId": "-100node"
    }));

    let resolved = resolve_telegram_chat_id(&telegram, &node).expect("chat id should resolve");

    assert_eq!(resolved, "-100node");
}

#[test]
fn telegram_chat_id_falls_back_to_user_settings() {
    let telegram = TelegramConfig {
        bot_token: "settings-token".to_string(),
        chat_id: "-100settings".to_string(),
    };
    let node = telegram_node(json!({
        "chatId": "   "
    }));

    let resolved = resolve_telegram_chat_id(&telegram, &node).expect("chat id should resolve");

    assert_eq!(resolved, "-100settings");
}

#[test]
fn telegram_chat_id_errors_when_node_and_settings_are_empty() {
    let telegram = TelegramConfig {
        bot_token: "settings-token".to_string(),
        chat_id: String::new(),
    };
    let node = telegram_node(json!({}));

    let err = resolve_telegram_chat_id(&telegram, &node).expect_err("chat id should be required");

    assert!(err
        .to_string()
        .contains("requires chatId or telegram.chat_id for the current user"));
}

#[test]
fn telegram_bot_token_uses_current_user_settings() {
    let telegram = TelegramConfig {
        bot_token: "settings-token".to_string(),
        chat_id: "-100settings".to_string(),
    };
    let node = telegram_node(json!({
        "botToken": "legacy-token"
    }));

    let resolved = resolve_telegram_bot_token(&telegram, &node).expect("bot token should resolve");

    assert_eq!(resolved, "settings-token");
}

#[test]
fn telegram_bot_token_errors_without_user_settings_even_if_legacy_exists() {
    let telegram = TelegramConfig {
        bot_token: String::new(),
        chat_id: "-100settings".to_string(),
    };
    let node = telegram_node(json!({
        "botToken": "legacy-token"
    }));

    let err = resolve_telegram_bot_token(&telegram, &node)
        .expect_err("legacy bot token should be rejected");

    assert!(err
        .to_string()
        .contains("legacy botToken is no longer used"));
}
