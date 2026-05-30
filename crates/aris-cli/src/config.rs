//! ARIS persistent configuration.
//!
//! Stores API keys and model preferences in `~/.config/aris/config.json`.
//! Environment variables always take priority over saved config.

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const CONFIG_DIR: &str = ".config/aris";
const CONFIG_FILE: &str = "config.json";

/// Controls which env vars `apply_to_env_inner` is allowed to overwrite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ApplyMode {
    /// Only set env vars that are currently unset. Shell-provided vars win.
    IfMissing,
    /// Clear + re-apply all executor AND reviewer env vars. Used by REPL
    /// `/setup` where the user explicitly reconfigured everything.
    ForceAll,
    /// Clear + re-apply only executor env vars. Used by mid-launch setup,
    /// which only asks about executor auth; reviewer env vars set by the
    /// user's shell must be preserved.
    ForceExecutorOnly,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArisConfig {
    /// "anthropic" or "openai"
    #[serde(default)]
    pub executor_provider: Option<String>,
    #[serde(default)]
    pub executor_api_key: Option<String>,
    #[serde(default)]
    pub executor_base_url: Option<String>,
    #[serde(default)]
    pub executor_model: Option<String>,
    /// "gemini" or "openai"
    #[serde(default)]
    pub reviewer_provider: Option<String>,
    #[serde(default)]
    pub reviewer_api_key: Option<String>,
    #[serde(default)]
    pub reviewer_base_url: Option<String>,
    #[serde(default)]
    pub reviewer_model: Option<String>,
    /// "cn" or "en"
    #[serde(default)]
    pub language: Option<String>,
    /// Meta-logging level: "off", "metadata", or "content"
    #[serde(default)]
    pub meta_logging: Option<String>,
}

impl ArisConfig {
    fn config_path() -> PathBuf {
        let home = runtime::home_dir();
        PathBuf::from(home).join(CONFIG_DIR).join(CONFIG_FILE)
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if !path.exists() {
            return Self::default();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> io::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        fs::write(&path, json)
    }

    /// Apply saved config to environment variables.
    /// Only sets vars that are currently unset or empty — shell-provided vars
    /// always win. Used at startup before we know what auth the user has.
    pub fn apply_to_env(&self) {
        self.apply_to_env_inner(ApplyMode::IfMissing);
    }

    /// Full clear + re-apply of both executor AND reviewer env vars.
    /// Used by REPL `/setup` where the user explicitly reconfigured everything.
    pub fn force_apply_to_env(&self) {
        self.apply_to_env_inner(ApplyMode::ForceAll);
    }

    /// Clear + re-apply only executor env vars; leave reviewer env vars alone.
    /// Used by the mid-launch setup wizard, which only asks about executor auth
    /// when that auth is missing. A shell-provided reviewer key (e.g.
    /// `OPENAI_API_KEY` for the reviewer) must not be wiped just because the
    /// user typed in an Anthropic executor key.
    pub fn force_apply_executor_env(&self) {
        self.apply_to_env_inner(ApplyMode::ForceExecutorOnly);
    }

    fn apply_to_env_inner(&self, mode: ApplyMode) {
        let force_exec = matches!(mode, ApplyMode::ForceAll | ApplyMode::ForceExecutorOnly);
        let force_rev = matches!(mode, ApplyMode::ForceAll);

        if force_exec {
            // Clear executor-related env vars to prevent cross-contamination
            // between providers when switching.
            std::env::remove_var("EXECUTOR_PROVIDER");
            std::env::remove_var("EXECUTOR_API_KEY");
            std::env::remove_var("EXECUTOR_BASE_URL");
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
            std::env::remove_var("ANTHROPIC_BASE_URL");
            // `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS` is executor-scoped (it
            // controls whether the Anthropic client attaches beta headers),
            // so it belongs in the executor clear block, not the reviewer one.
            std::env::remove_var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS");
        }
        if force_rev {
            // Clear reviewer-related env vars — only when user explicitly
            // reconfigured reviewer via REPL /setup. NOT cleared by mid-launch
            // executor-only setup, to preserve shell-provided reviewer keys.
            std::env::remove_var("OPENAI_API_KEY");
            std::env::remove_var("GEMINI_API_KEY");
            std::env::remove_var("GLM_API_KEY");
            std::env::remove_var("MINIMAX_API_KEY");
            std::env::remove_var("KIMI_API_KEY");
            std::env::remove_var("ARIS_REVIEWER_MODEL");
            std::env::remove_var("ARIS_REVIEWER_BASE_URL");
            std::env::remove_var("ARIS_REVIEWER_PROVIDER");
            std::env::remove_var("ARIS_REVIEWER_AUTH_TOKEN");
        }
        // The rest of the function uses `force_exec` and `force_rev` to decide
        // whether to overwrite existing env vars.
        let force = force_exec;
        let force_reviewer = force_rev;

        if let Some(provider) = &self.executor_provider {
            if provider == "openai" || provider == "custom" {
                std::env::set_var("EXECUTOR_PROVIDER", "openai");
            }
        }

        // Executor API key + base URL
        let provider = self.executor_provider.as_deref().unwrap_or("anthropic");
        if let Some(key) = &self.executor_api_key {
            match provider {
                "anthropic" => {
                    if force || std::env::var("ANTHROPIC_API_KEY").is_err() {
                        std::env::set_var("ANTHROPIC_API_KEY", key);
                    }
                    if let Some(url) = &self.executor_base_url {
                        if force || std::env::var("ANTHROPIC_BASE_URL").is_err() {
                            std::env::set_var("ANTHROPIC_BASE_URL", url);
                        }
                        // Third-party providers may reject Anthropic-specific beta flags
                        if force || std::env::var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS").is_err()
                        {
                            std::env::set_var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS", "1");
                        }
                    }
                }
                "anthropic-compat" => {
                    // MiniMax etc: Anthropic-compatible endpoint with bearer token
                    if force || std::env::var("ANTHROPIC_AUTH_TOKEN").is_err() {
                        std::env::set_var("ANTHROPIC_AUTH_TOKEN", key);
                    }
                    if let Some(url) = &self.executor_base_url {
                        if force || std::env::var("ANTHROPIC_BASE_URL").is_err() {
                            std::env::set_var("ANTHROPIC_BASE_URL", url);
                        }
                        // Third-party providers may reject Anthropic-specific beta flags
                        if force || std::env::var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS").is_err()
                        {
                            std::env::set_var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS", "1");
                        }
                    }
                }
                "openai" | "custom" => {
                    if force || std::env::var("EXECUTOR_API_KEY").is_err() {
                        std::env::set_var("EXECUTOR_API_KEY", key);
                    }
                }
                _ => {}
            }
        }

        // Executor base URL (for openai-compat providers)
        if provider == "openai" || provider == "custom" {
            if force || std::env::var("EXECUTOR_BASE_URL").is_err() {
                if let Some(url) = &self.executor_base_url {
                    std::env::set_var("EXECUTOR_BASE_URL", url);
                }
            }
        }

        // Reviewer API key — gated on force_reviewer, not force_exec, so
        // executor-only force does not clobber shell-provided reviewer keys.
        if let Some(reviewer_provider) = &self.reviewer_provider {
            if let Some(key) = &self.reviewer_api_key {
                match reviewer_provider.as_str() {
                    "gemini" => {
                        if force_reviewer || std::env::var("GEMINI_API_KEY").is_err() {
                            std::env::set_var("GEMINI_API_KEY", key);
                        }
                    }
                    "openai" => {
                        if force_reviewer || std::env::var("OPENAI_API_KEY").is_err() {
                            std::env::set_var("OPENAI_API_KEY", key);
                        }
                    }
                    "glm" => {
                        if force_reviewer || std::env::var("GLM_API_KEY").is_err() {
                            std::env::set_var("GLM_API_KEY", key);
                        }
                    }
                    "minimax" => {
                        if force_reviewer || std::env::var("MINIMAX_API_KEY").is_err() {
                            std::env::set_var("MINIMAX_API_KEY", key);
                        }
                    }
                    "kimi" => {
                        if force_reviewer || std::env::var("KIMI_API_KEY").is_err() {
                            std::env::set_var("KIMI_API_KEY", key);
                        }
                    }
                    "anthropic-compat" => {
                        if force_reviewer || std::env::var("ARIS_REVIEWER_AUTH_TOKEN").is_err() {
                            std::env::set_var("ARIS_REVIEWER_AUTH_TOKEN", key);
                        }
                    }
                    "deepseek" => {
                        if force_reviewer || std::env::var("ARIS_REVIEWER_AUTH_TOKEN").is_err() {
                            std::env::set_var("ARIS_REVIEWER_AUTH_TOKEN", key);
                        }
                    }
                    "custom" => {
                        // Custom OpenAI-compatible reviewer: store key in
                        // ARIS_REVIEWER_AUTH_TOKEN so it doesn't collide with
                        // the executor's OPENAI_API_KEY.
                        if force_reviewer || std::env::var("ARIS_REVIEWER_AUTH_TOKEN").is_err() {
                            std::env::set_var("ARIS_REVIEWER_AUTH_TOKEN", key);
                        }
                    }
                    _ => {}
                }
            }
            // Set reviewer provider env var
            if force_reviewer || std::env::var("ARIS_REVIEWER_PROVIDER").is_err() {
                std::env::set_var("ARIS_REVIEWER_PROVIDER", reviewer_provider);
            }
        }

        // Reviewer base URL
        if force_reviewer || std::env::var("ARIS_REVIEWER_BASE_URL").is_err() {
            if let Some(url) = &self.reviewer_base_url {
                std::env::set_var("ARIS_REVIEWER_BASE_URL", url);
            }
        }

        // Reviewer model
        if force_reviewer || std::env::var("ARIS_REVIEWER_MODEL").is_err() {
            if let Some(model) = &self.reviewer_model {
                std::env::set_var("ARIS_REVIEWER_MODEL", model);
            }
        }

        // Language
        if force || std::env::var("ARIS_LANGUAGE").is_err() {
            if let Some(lang) = &self.language {
                std::env::set_var("ARIS_LANGUAGE", lang);
            }
        }

        // Meta-logging
        if force || std::env::var("ARIS_META_LOGGING").is_err() {
            if let Some(level) = &self.meta_logging {
                std::env::set_var("ARIS_META_LOGGING", level);
            }
        }
    }

    /// Returns the executor model from config, or None.
    pub fn executor_model(&self) -> Option<&str> {
        self.executor_model.as_deref()
    }
}

/// Interactive setup wizard. Returns the configured settings.
pub fn run_interactive_setup() -> io::Result<ArisConfig> {
    let mut config = ArisConfig::load();

    println!("\x1b[1mARIS Setup\x1b[0m");
    println!("\x1b[2mConfigure API keys and models. Press Enter to keep current value.\x1b[0m\n");

    // ── Step 1+2: Executor provider + key + model ──
    println!("\x1b[1m[1/3] Executor (main LLM)\x1b[0m");
    println!("  1. Anthropic   (claude-opus / sonnet / haiku)");
    println!("  2. OpenAI      (gpt-5.5)");
    println!("  3. Gemini      (gemini-2.5-pro)");
    println!("  4. GLM         (GLM-5)");
    println!("  5. MiniMax     (MiniMax-M2.7)");
    println!("  6. Kimi        (kimi-k2.5)");
    println!("  7. DeepSeek    (deepseek-v4-pro)");
    println!("  8. Xiaomi      (mimo-v2.5-pro)");
    println!("  9. Qwen        (qwen3.6-plus)");
    println!(" 10. Doubao      (doubao-pro-4k)");
    println!(" 11. Custom      (OpenAI-compatible endpoint)");

    let default_executor = match config.executor_provider.as_deref() {
        Some("anthropic") => "1",
        Some("anthropic-compat") => match config.executor_base_url.as_deref() {
            Some(u) if u.contains("deepseek") => "7",
            _ => "1",
        },
        Some("custom") => "11",
        Some("openai") => match config.executor_base_url.as_deref() {
            Some(u) if u.contains("googleapis") => "3",
            Some(u) if u.contains("bigmodel") => "4",
            Some(u) if u.contains("minimax") => "5",
            Some(u) if u.contains("moonshot") => "6",
            Some(u) if u.contains("xiaomimimo") => "8",
            Some(u) if u.contains("dashscope") => "9",
            Some(u) if u.contains("volces") => "10",
            _ => "2",
        },
        _ => "1",
    };
    let exec_choice_raw = prompt_with_default("  Choose [1-11]", default_executor)?;
    let exec_choice = exec_choice_raw.trim();
    // Detect real menu change, not just provider-string change. OpenAI / Gemini /
    // GLM / MiniMax / Kimi all serialize to provider="openai" so we must compare
    // the menu choice to catch switches like "OpenAI → Kimi" properly.
    let switched_executor = exec_choice != default_executor;

    // (provider, key_env, key_label, base_url, default_model)
    let exec_info: (&str, &str, &str, Option<&str>, &str) = match exec_choice {
        "2" => (
            "openai",
            "EXECUTOR_API_KEY",
            "OpenAI API key",
            Some("https://api.openai.com/v1"),
            "gpt-5.5",
        ),
        "3" => (
            "openai",
            "EXECUTOR_API_KEY",
            "Gemini API key",
            Some("https://generativelanguage.googleapis.com/v1beta/openai"),
            "gemini-2.5-pro",
        ),
        "4" => (
            "openai",
            "EXECUTOR_API_KEY",
            "GLM API key",
            Some("https://open.bigmodel.cn/api/paas/v4"),
            "GLM-5",
        ),
        "5" => (
            "openai",
            "EXECUTOR_API_KEY",
            "MiniMax API key",
            Some("https://api.minimax.chat/v1"),
            "MiniMax-M2.7",
        ),
        "6" => (
            "openai",
            "EXECUTOR_API_KEY",
            "Kimi API key",
            Some("https://api.moonshot.cn/v1"),
            "kimi-k2.5",
        ),
        "7" => (
            "anthropic-compat",
            "ANTHROPIC_AUTH_TOKEN",
            "DeepSeek API key",
            Some("https://api.deepseek.com/anthropic"),
            "deepseek-v4-pro",
        ),
        "8" => (
            "openai",
            "EXECUTOR_API_KEY",
            "Xiaomi API key",
            Some("https://token-plan-cn.xiaomimimo.com/v1"),
            "mimo-v2.5-pro",
        ),
        "9" => (
            "openai",
            "EXECUTOR_API_KEY",
            "Qwen (DashScope) API key",
            Some("https://dashscope.aliyuncs.com/compatible-mode/v1"),
            "qwen3.6-plus",
        ),
        "10" => (
            "openai",
            "EXECUTOR_API_KEY",
            "Doubao (Ark) API key",
            Some("https://ark.cn-beijing.volces.com/api/v3"),
            "doubao-pro-4k",
        ),
        "11" => ("custom", "EXECUTOR_API_KEY", "API key", None, ""),
        _ => (
            "anthropic",
            "ANTHROPIC_API_KEY",
            "Anthropic API key",
            None,
            "claude-opus-4-7",
        ),
    };

    // Preserve an explicit `anthropic-compat` choice across re-runs of `/setup`.
    // Menu option 1 covers both `anthropic` (x-api-key) and `anthropic-compat`
    // (Bearer) — if the user had Bearer mode set previously (e.g. for a proxy
    // that requires it) and stays on option 1, we must NOT silently downgrade
    // them to `anthropic`. Switching menu options obviously resets this.
    let prev_provider = config.executor_provider.as_deref();
    let target_provider = if !switched_executor
        && exec_info.0 == "anthropic"
        && prev_provider == Some("anthropic-compat")
    {
        "anthropic-compat"
    } else {
        exec_info.0
    };
    config.executor_provider = Some(target_provider.into());

    // Only overwrite base_url + clear stale key when user actually switched
    // to a different menu option. If they stayed on the same option, preserve
    // any custom base_url they typed previously (e.g. OpenRouter, newcli.com
    // proxy). Previously we always overwrote the URL to the provider's built-in
    // default, which silently wiped custom URLs between setup runs.
    if switched_executor {
        if let Some(url) = exec_info.3 {
            config.executor_base_url = Some(url.into());
        } else {
            config.executor_base_url = None;
        }
        config.executor_api_key = None;
        // Clear stale model on menu switch. For built-in providers the next
        // line overwrites this with `exec_info.4` anyway, but for the Custom
        // option this matters: otherwise switching from OpenAI/Gemini → Custom
        // would carry forward `gpt-5.5` / `gemini-2.5-pro` as the "current"
        // custom model, and the post-fetch fallback prompt (which only fires
        // when executor_model is empty) would be skipped.
        config.executor_model = None;
    }

    // Ask for API key
    let current_key_masked = config
        .executor_api_key
        .as_deref()
        .filter(|k| k.len() > 8)
        .map(|k| format!("{}...{}", &k[..4], &k[k.len() - 4..]))
        .unwrap_or_else(|| "(not set)".into());
    let new_key = prompt_with_default(&format!("  {} [{current_key_masked}]", exec_info.2), "")?;
    if !new_key.is_empty() {
        config.executor_api_key = Some(new_key);
    }

    // Show known-working proxy URLs before the prompt (provider-aware).
    print_executor_url_hints(exec_choice);

    // Ask for proxy/custom base URL (all providers). The prompt text says
    // "Enter to keep" — pressing Enter preserves the current value, it does
    // NOT reset to the provider's official default. To switch back to the
    // official endpoint, type the URL explicitly.
    let current_url_hint = config
        .executor_base_url
        .as_deref()
        .unwrap_or("(none — uses official default)");
    let custom_url = prompt_with_default(
        &format!("  Proxy base URL [{current_url_hint}] (Enter to keep)"),
        "",
    )?;
    if !custom_url.is_empty() {
        config.executor_base_url = Some(custom_url.clone());
    }
    // NOTE (v0.4.4): Removed the auto-switch from "anthropic" to
    // "anthropic-compat" when a custom URL was entered. Anthropic-format
    // proxies like code.newcli.com/claude and api-inference.modelscope.cn
    // accept `x-api-key` (which the `anthropic` provider path sends), not
    // `Authorization: Bearer` (which `anthropic-compat` forces) — the old
    // auto-switch made issues #158 and #162 unreachable via the UI.

    // Auto-set best model for the chosen provider
    if exec_choice == "11" {
        // Custom provider: try fetching available models from /models endpoint
        let api_key = config.executor_api_key.as_deref().unwrap_or("");
        let base_url = config.executor_base_url.as_deref().unwrap_or("");
        if !api_key.is_empty() && !base_url.is_empty() {
            println!("  \x1b[2mFetching models from {base_url}...\x1b[0m");
            match crate::openai_compat::fetch_openai_models(base_url, api_key) {
                Ok(models) => {
                    let current = config.executor_model.as_deref().unwrap_or("");
                    let items = crate::openai_compat::model_select_items(&models, current);
                    match crate::input::select_menu(
                        "Select model",
                        "Choose a model from the provider's /models endpoint.",
                        &items,
                    ) {
                        Ok(Some(idx)) => {
                            config.executor_model = Some(items[idx].label.clone());
                        }
                        Ok(None) => {
                            // User cancelled — keep existing model
                        }
                        Err(_) => {
                            // select_menu I/O error — fall through to manual
                        }
                    }
                }
                Err(err) => {
                    println!("  \x1b[33m⚠ Could not fetch models: {err}\x1b[0m");
                    println!("  \x1b[2mYou can type the model name manually below.\x1b[0m");
                }
            }
        }
        // If no model set yet (fetch failed or user has no key/url), ask manually
        if config.executor_model.as_deref().unwrap_or("").is_empty() {
            let current_model_hint = config.executor_model.as_deref().unwrap_or("(not set)");
            let custom_model = prompt_with_default(
                &format!("  Model name [{current_model_hint}]"),
                config.executor_model.as_deref().unwrap_or(""),
            )?;
            if !custom_model.is_empty() {
                config.executor_model = Some(custom_model.clone());
            }
        }
        println!(
            "  \x1b[2mModel: {}\x1b[0m",
            config.executor_model.as_deref().unwrap_or("(none)")
        );
    } else {
        config.executor_model = Some(exec_info.4.to_string());
        println!("  \x1b[2mModel: {}\x1b[0m", exec_info.4);
    }

    // ── Step 4: Reviewer ──
    println!("\n\x1b[1m[2/3] Reviewer (for LlmReview tool)\x1b[0m");
    println!("  1. OpenAI          (gpt-5.5)");
    println!("  2. Gemini          (gemini-2.5-pro)");
    println!("  3. GLM             (GLM-5)");
    println!("  4. MiniMax         (MiniMax-M2.7)");
    println!("  5. Kimi            (kimi-k2.5)");
    println!("  6. Anthropic Proxy (claude via proxy)");
    println!("  7. DeepSeek        (deepseek-v4-pro)");
    println!("  8. Skip (no reviewer)");
    println!("  9. Custom          (OpenAI-compatible endpoint)");
    let default_reviewer = match config.reviewer_provider.as_deref() {
        Some("openai") => "1",
        Some("gemini") => "2",
        Some("glm") => "3",
        Some("minimax") => "4",
        Some("kimi") => "5",
        Some("anthropic-compat") => "6",
        Some("deepseek") => "7",
        Some("custom") => "9",
        None => "1",
        _ => "8",
    };
    let reviewer_choice_raw = prompt_with_default("  Choose [1-9]", default_reviewer)?;
    let reviewer_choice = reviewer_choice_raw.trim();
    let switched_reviewer = reviewer_choice != default_reviewer;

    // (provider_name, key_env_var, key_label, default_model)
    let reviewer_info: Option<(&str, &str, &str, &str)> = match reviewer_choice {
        "1" => Some(("openai", "OPENAI_API_KEY", "OpenAI API key", "gpt-5.5")),
        "2" => Some((
            "gemini",
            "GEMINI_API_KEY",
            "Gemini API key",
            "gemini-2.5-pro",
        )),
        "3" => Some(("glm", "GLM_API_KEY", "GLM API key", "GLM-5")),
        "4" => Some((
            "minimax",
            "MINIMAX_API_KEY",
            "MiniMax API key",
            "MiniMax-M2.7",
        )),
        "5" => Some(("kimi", "KIMI_API_KEY", "Kimi API key", "kimi-k2.5")),
        "6" => Some((
            "anthropic-compat",
            "ARIS_REVIEWER_AUTH_TOKEN",
            "Reviewer auth token",
            "claude-sonnet-4-6",
        )),
        "7" => Some((
            "deepseek",
            "ARIS_REVIEWER_AUTH_TOKEN",
            "DeepSeek API key",
            "deepseek-v4-pro",
        )),
        "9" => Some(("custom", "ARIS_REVIEWER_AUTH_TOKEN", "API key", "")),
        _ => None,
    };

    if let Some((provider, key_env, key_label, default_model)) = reviewer_info {
        config.reviewer_provider = Some(provider.into());
        // Clear stale reviewer state when switching menu option. Without this,
        // e.g. Kimi → OpenAI leaves the moonshot URL saved as reviewer_base_url
        // and the old Kimi key as reviewer_api_key — both get shown as
        // "current" values for the new OpenAI provider, producing confused
        // configs (seen in issue #158 testing).
        if switched_reviewer {
            config.reviewer_api_key = None;
            config.reviewer_base_url = None;
            // Same reasoning as the executor switch above: clear stale model so
            // the Custom-reviewer fetch-failure fallback prompt actually fires.
            config.reviewer_model = None;
        }

        // Ask for API key
        let current_masked = std::env::var(key_env)
            .ok()
            .or_else(|| config.reviewer_api_key.clone())
            .filter(|k| k.len() > 8)
            .map(|k| format!("{}...{}", &k[..4], &k[k.len() - 4..]))
            .unwrap_or_else(|| "(not set)".into());
        let new_key = prompt_with_default(&format!("  {key_label} [{current_masked}]"), "")?;
        if !new_key.is_empty() {
            config.reviewer_api_key = Some(new_key.clone());
            std::env::set_var(key_env, &new_key);
        } else if let Some(existing) = &config.reviewer_api_key {
            std::env::set_var(key_env, existing);
        }

        // Show known-working proxy URLs before the prompt (provider-aware).
        print_reviewer_url_hints(reviewer_choice);

        // Ask for proxy/custom base URL for reviewer
        let current_reviewer_url = config
            .reviewer_base_url
            .as_deref()
            .unwrap_or("(none — uses official default)");
        let custom_reviewer_url = prompt_with_default(
            &format!("  Proxy base URL [{current_reviewer_url}] (Enter to keep)"),
            "",
        )?;
        if !custom_reviewer_url.is_empty() {
            config.reviewer_base_url = Some(custom_reviewer_url);
        }

        // Auto-set best model for the chosen reviewer provider
        // v0.4.8 fix: Custom is menu option 9, not 8 (8 is "Skip"). The
        // previous "8" check meant Custom fell through to the else branch
        // (`reviewer_model = Some(default_model)` = `Some("")` since custom's
        // default_model is the empty string), which then persisted in
        // config.json and caused every reboot to reset reviewer to the
        // gpt-5.5 fallback chain in main.rs.
        if reviewer_choice == "9" {
            // Custom provider: try fetching available models from /models endpoint
            let api_key = config.reviewer_api_key.as_deref().unwrap_or("");
            let base_url = config.reviewer_base_url.as_deref().unwrap_or("");
            if !api_key.is_empty() && !base_url.is_empty() {
                println!("  \x1b[2mFetching models from {base_url}...\x1b[0m");
                match crate::openai_compat::fetch_openai_models(base_url, api_key) {
                    Ok(models) => {
                        let current = config.reviewer_model.as_deref().unwrap_or("");
                        let items = crate::openai_compat::model_select_items(&models, current);
                        match crate::input::select_menu(
                            "Select reviewer model",
                            "Choose a model from the provider's /models endpoint.",
                            &items,
                        ) {
                            Ok(Some(idx)) => {
                                config.reviewer_model = Some(items[idx].label.clone());
                            }
                            Ok(None) => {}
                            Err(_) => {}
                        }
                    }
                    Err(err) => {
                        println!("  \x1b[33m⚠ Could not fetch models: {err}\x1b[0m");
                        println!("  \x1b[2mYou can type the model name manually below.\x1b[0m");
                    }
                }
            }
            // If no model set yet, ask manually
            if config.reviewer_model.as_deref().unwrap_or("").is_empty() {
                let current_model_hint = config.reviewer_model.as_deref().unwrap_or("(not set)");
                let custom_model = prompt_with_default(
                    &format!("  Model name [{current_model_hint}]"),
                    config.reviewer_model.as_deref().unwrap_or(""),
                )?;
                if !custom_model.is_empty() {
                    config.reviewer_model = Some(custom_model.clone());
                }
            }
            println!(
                "  \x1b[2mModel: {}\x1b[0m",
                config.reviewer_model.as_deref().unwrap_or("(none)")
            );
        } else {
            config.reviewer_model = Some(default_model.to_string());
            println!("  \x1b[2mModel: {default_model}\x1b[0m");
        }
    } else {
        config.reviewer_provider = None;
        config.reviewer_api_key = None;
        config.reviewer_base_url = None;
        config.reviewer_model = None;
    }

    // ── Step 5: Language ──
    println!("\n\x1b[1m[3/3] Language\x1b[0m");
    println!("  1. 中文 (CN)");
    println!("  2. English (EN)");
    let lang_choice = prompt_with_default(
        "  Choose [1/2]",
        match config.language.as_deref() {
            Some("en") => "2",
            _ => "1",
        },
    )?;
    config.language = Some(
        if lang_choice.trim() == "2" {
            "en"
        } else {
            "cn"
        }
        .into(),
    );

    // ── Save ──
    println!("\n\x1b[1mSaving configuration\x1b[0m");
    config.save()?;
    let path = ArisConfig::config_path();
    println!("  Saved to {}", path.display());

    println!("\n\x1b[1;32m✓ Setup complete!\x1b[0m Run `aris` to start.\n");

    Ok(config)
}

/// Print a provider-specific list of known-working third-party proxy URLs
/// before the executor URL prompt. Keeps the input-URL flow unchanged —
/// this is pure UX (helps users know what to type for OpenRouter, ModelScope,
/// etc.) and costs nothing if the user doesn't care.
///
/// Examples are restricted to URLs we've actually validated or seen reported
/// working in issues (#158, #162, etc.). Avoid listing proxies that need
/// transport-specific headers we don't implement yet (e.g. DashScope Coding
/// Plan under Anthropic — issue #159 — requires a specific header).
fn print_executor_url_hints(exec_choice: &str) {
    match exec_choice {
        "1" => {
            // Anthropic: official api.anthropic.com or an Anthropic-format proxy.
            println!(
                "  \x1b[2mProxy examples (leave blank for official api.anthropic.com):\x1b[0m"
            );
            println!("    \x1b[2m• https://code.newcli.com/claude        (Claude-Code-compatible proxy)\x1b[0m");
            println!("    \x1b[2m• https://api-inference.modelscope.cn   (ModelScope Anthropic endpoint)\x1b[0m");
        }
        "2" => {
            // OpenAI (vanilla) or OpenAI-format proxy.
            println!("  \x1b[2mProxy examples (leave blank for official api.openai.com):\x1b[0m");
            println!("    \x1b[2m• https://openrouter.ai/api/v1                        (OpenRouter)\x1b[0m");
            println!("    \x1b[2m• https://api.deepseek.com/v1                         (DeepSeek)\x1b[0m");
            println!("    \x1b[2m• https://dashscope.aliyuncs.com/compatible-mode/v1   (阿里云百练 OpenAI-compat)\x1b[0m");
        }
        "7" => {
            // DeepSeek via Anthropic-compatible API (supports extended thinking).
            println!("  \x1b[2mDeepSeek Anthropic-compatible endpoint:\x1b[0m");
            println!("    \x1b[2m• https://api.deepseek.com/anthropic                       (official)\x1b[0m");
        }
        "9" => {
            // Qwen: DashScope has both standard and Coding Plan endpoints.
            println!("  \x1b[2mProxy examples (leave blank for official DashScope):\x1b[0m");
            println!("    \x1b[2m• https://coding.dashscope.aliyuncs.com/v1               (百炼 Coding Plan)\x1b[0m");
        }
        _ => {}
    }
}

/// Print provider-specific proxy URL hints for the reviewer menu. v0.4.4
/// only covers OpenAI-format reviewer proxies; anthropic-compat reviewer
/// still sends Bearer-only (separate fix planned), so `code.newcli.com`-
/// style proxies that require x-api-key aren't listed under option 6.
fn print_reviewer_url_hints(reviewer_choice: &str) {
    match reviewer_choice {
        "1" => {
            println!("  \x1b[2mProxy examples (leave blank for official api.openai.com):\x1b[0m");
            println!("    \x1b[2m• https://openrouter.ai/api/v1                        (OpenRouter)\x1b[0m");
            println!("    \x1b[2m• https://api.deepseek.com/v1                         (DeepSeek)\x1b[0m");
            println!("    \x1b[2m• https://dashscope.aliyuncs.com/compatible-mode/v1   (阿里云百练 OpenAI-compat)\x1b[0m");
        }
        "7" => {
            println!("  \x1b[2mDeepSeek Anthropic-compatible endpoint:\x1b[0m");
            println!("    \x1b[2m• https://api.deepseek.com/anthropic                       (official)\x1b[0m");
        }
        _ => {}
    }
}

fn prompt_with_default(prompt: &str, default: &str) -> io::Result<String> {
    print!("{prompt}: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim().to_string();
    if trimmed.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Env mutation is serialized across the whole crate test binary via the
    // shared `crate::env_test_guard()` (codex Phase-0 gap #1) so config.rs and
    // openai_executor.rs env tests cannot race on EXECUTOR_*/OPENAI_API_KEY.

    struct EnvSnapshot {
        vars: Vec<(&'static str, Option<String>)>,
    }

    impl EnvSnapshot {
        fn capture(names: &[&'static str]) -> Self {
            let vars = names.iter().map(|n| (*n, std::env::var(n).ok())).collect();
            // Clear them so the test starts from a known state.
            for n in names {
                std::env::remove_var(n);
            }
            Self { vars }
        }
    }

    impl Drop for EnvSnapshot {
        fn drop(&mut self) {
            for (name, prior) in &self.vars {
                match prior {
                    Some(v) => std::env::set_var(name, v),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    const EXECUTOR_ENV_VARS: &[&str] = &[
        "EXECUTOR_PROVIDER",
        "EXECUTOR_API_KEY",
        "EXECUTOR_BASE_URL",
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_BASE_URL",
        "CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS",
    ];

    #[test]
    fn anthropic_with_custom_base_url_sets_base_url_and_disables_betas() {
        let _g = crate::env_test_guard();
        let _snap = EnvSnapshot::capture(EXECUTOR_ENV_VARS);

        let config = ArisConfig {
            executor_provider: Some("anthropic".into()),
            executor_api_key: Some("sk-ant-test".into()),
            executor_base_url: Some("https://bedrock-proxy.example.com".into()),
            ..Default::default()
        };
        config.force_apply_to_env();

        assert_eq!(
            std::env::var("ANTHROPIC_API_KEY").ok().as_deref(),
            Some("sk-ant-test")
        );
        assert_eq!(
            std::env::var("ANTHROPIC_BASE_URL").ok().as_deref(),
            Some("https://bedrock-proxy.example.com")
        );
        assert_eq!(
            std::env::var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS")
                .ok()
                .as_deref(),
            Some("1")
        );
    }

    #[test]
    fn anthropic_without_custom_base_url_leaves_betas_enabled() {
        let _g = crate::env_test_guard();
        let _snap = EnvSnapshot::capture(EXECUTOR_ENV_VARS);

        let config = ArisConfig {
            executor_provider: Some("anthropic".into()),
            executor_api_key: Some("sk-ant-test".into()),
            executor_base_url: None,
            ..Default::default()
        };
        config.force_apply_to_env();

        // Official api.anthropic.com path: no base URL override, betas stay on.
        assert!(std::env::var("ANTHROPIC_BASE_URL").is_err());
        assert!(std::env::var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS").is_err());
    }

    #[test]
    fn anthropic_compat_with_base_url_sets_auth_token_base_url_and_disables_betas() {
        let _g = crate::env_test_guard();
        let _snap = EnvSnapshot::capture(EXECUTOR_ENV_VARS);

        let config = ArisConfig {
            executor_provider: Some("anthropic-compat".into()),
            executor_api_key: Some("mx-token".into()),
            executor_base_url: Some("https://minimax.example.com/anthropic".into()),
            ..Default::default()
        };
        config.force_apply_to_env();

        assert_eq!(
            std::env::var("ANTHROPIC_AUTH_TOKEN").ok().as_deref(),
            Some("mx-token")
        );
        assert_eq!(
            std::env::var("ANTHROPIC_BASE_URL").ok().as_deref(),
            Some("https://minimax.example.com/anthropic")
        );
        assert_eq!(
            std::env::var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS")
                .ok()
                .as_deref(),
            Some("1")
        );
    }

    #[test]
    fn force_apply_executor_env_clears_stale_beta_disable_flag() {
        let _g = crate::env_test_guard();
        let _snap = EnvSnapshot::capture(EXECUTOR_ENV_VARS);

        // Simulate a prior run that had a custom base URL and thus set the flag.
        std::env::set_var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS", "1");
        std::env::set_var("ANTHROPIC_BASE_URL", "https://old-proxy.example.com");

        // User then reconfigured to official api.anthropic.com (no base URL).
        let config = ArisConfig {
            executor_provider: Some("anthropic".into()),
            executor_api_key: Some("sk-ant-test".into()),
            executor_base_url: None,
            ..Default::default()
        };
        config.force_apply_executor_env();

        // Stale flags from the prior custom-URL run must be gone, otherwise
        // the Anthropic client would keep stripping beta headers against the
        // official API and we'd lose OAuth/long-context/interleaved-thinking.
        assert!(
            std::env::var("ANTHROPIC_BASE_URL").is_err(),
            "expected ANTHROPIC_BASE_URL to be cleared by force_apply_executor_env"
        );
        assert!(
            std::env::var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS").is_err(),
            "expected CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS to be cleared too"
        );
    }

    // ─────────────────────────────────────────────────────────────────────
    // v0.4.16 Phase 0 — CHARACTERIZATION (golden master) tests.
    //
    // These lock the CURRENT behavior of `apply_to_env_inner` before the
    // P7 ProviderFamily refactor. They are NOT specifications of what the
    // code SHOULD do — they pin what it ACTUALLY does today so any
    // behavior change during the refactor is caught immediately. If one of
    // these fails after a refactor, that is a REGRESSION, not a stale
    // assertion: the env-writing contract these providers rely on changed.
    //
    // Env isolation: every test below takes crate::env_test_guard() + EnvSnapshot::capture
    // (save/clear/restore) exactly like the pre-existing tests above.
    // `apply_to_env_inner` reads only `&self` + process env (never disk),
    // so no HOME/config-file isolation is needed.
    // ─────────────────────────────────────────────────────────────────────

    /// case: exec_anthropic_official_endpoint
    /// Locks: executor_provider="anthropic" + base_url=None (official
    /// api.anthropic.com path). The highest-priority Category-A invariant —
    /// ANTHROPIC_API_KEY (x-api-key auth) is set, NO base URL override is
    /// written, betas stay ON (CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS unset),
    /// and EXECUTOR_PROVIDER is NOT set (so resolve_openai_executor_config
    /// returns None → Anthropic client path).
    #[test]
    fn char_exec_anthropic_official_endpoint() {
        let _g = crate::env_test_guard();
        let _snap = EnvSnapshot::capture(EXECUTOR_ENV_VARS);

        let config = ArisConfig {
            executor_provider: Some("anthropic".into()),
            executor_api_key: Some("K".into()),
            executor_base_url: None,
            ..Default::default()
        };
        config.force_apply_to_env();

        assert_eq!(std::env::var("ANTHROPIC_API_KEY").ok().as_deref(), Some("K"));
        // Official endpoint: no base URL, no beta-disable, betas remain ON.
        assert!(std::env::var("ANTHROPIC_BASE_URL").is_err());
        assert!(std::env::var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS").is_err());
        // anthropic path never sets EXECUTOR_PROVIDER → OpenAI resolver = None.
        assert!(std::env::var("EXECUTOR_PROVIDER").is_err());
        // anthropic path never sets ANTHROPIC_AUTH_TOKEN (that's the
        // anthropic-compat Bearer path).
        assert!(std::env::var("ANTHROPIC_AUTH_TOKEN").is_err());
    }

    /// case: exec_anthropic_custom_url_keeps_xapikey  🔴 HIGHEST-PRIORITY GUARD
    /// Locks the #158/#162 regression: executor_provider="anthropic" with a
    /// CUSTOM base_url must keep x-api-key auth (ANTHROPIC_API_KEY), and must
    /// NOT silently switch to the anthropic-compat Bearer path
    /// (ANTHROPIC_AUTH_TOKEN). Anthropic-format proxies (code.newcli.com/claude,
    /// modelscope) accept x-api-key, NOT `Authorization: Bearer`. Custom URL
    /// DOES set ANTHROPIC_BASE_URL and disables betas (third-party may reject
    /// Anthropic beta flags). This is the single most refactor-fragile route.
    #[test]
    fn char_exec_anthropic_custom_url_keeps_xapikey() {
        let _g = crate::env_test_guard();
        let _snap = EnvSnapshot::capture(EXECUTOR_ENV_VARS);

        let config = ArisConfig {
            executor_provider: Some("anthropic".into()),
            executor_api_key: Some("K".into()),
            executor_base_url: Some("https://code.newcli.com/claude".into()),
            ..Default::default()
        };
        config.force_apply_to_env();

        // x-api-key auth preserved — the load-bearing assertion.
        assert_eq!(std::env::var("ANTHROPIC_API_KEY").ok().as_deref(), Some("K"));
        // Must NOT have flipped to Bearer (anthropic-compat) auth.
        assert!(
            std::env::var("ANTHROPIC_AUTH_TOKEN").is_err(),
            "#158/#162 regression: anthropic+custom URL must NOT set ANTHROPIC_AUTH_TOKEN"
        );
        assert_eq!(
            std::env::var("ANTHROPIC_BASE_URL").ok().as_deref(),
            Some("https://code.newcli.com/claude")
        );
        assert_eq!(
            std::env::var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS")
                .ok()
                .as_deref(),
            Some("1")
        );
        // Still Anthropic-client routed (no OpenAI EXECUTOR_PROVIDER).
        assert!(std::env::var("EXECUTOR_PROVIDER").is_err());
    }

    /// case: exec_anthropic_compat_bearer
    /// Locks the Bearer path: executor_provider="anthropic-compat" sets
    /// ANTHROPIC_AUTH_TOKEN (Bearer) — NOT ANTHROPIC_API_KEY (x-api-key) —
    /// plus base URL + beta-disable. This is the other side of the
    /// x-api-key vs Bearer bisection that #158/#162 turns on.
    #[test]
    fn char_exec_anthropic_compat_bearer() {
        let _g = crate::env_test_guard();
        let _snap = EnvSnapshot::capture(EXECUTOR_ENV_VARS);

        let config = ArisConfig {
            executor_provider: Some("anthropic-compat".into()),
            executor_api_key: Some("K".into()),
            executor_base_url: Some("https://api.deepseek.com/anthropic".into()),
            ..Default::default()
        };
        config.force_apply_to_env();

        // Bearer token, NOT x-api-key.
        assert_eq!(
            std::env::var("ANTHROPIC_AUTH_TOKEN").ok().as_deref(),
            Some("K")
        );
        assert!(
            std::env::var("ANTHROPIC_API_KEY").is_err(),
            "anthropic-compat must NOT set ANTHROPIC_API_KEY (x-api-key)"
        );
        assert_eq!(
            std::env::var("ANTHROPIC_BASE_URL").ok().as_deref(),
            Some("https://api.deepseek.com/anthropic")
        );
        assert_eq!(
            std::env::var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS")
                .ok()
                .as_deref(),
            Some("1")
        );
        assert!(std::env::var("EXECUTOR_PROVIDER").is_err());
    }

    /// case: exec_anthropic_compat_no_baseurl_edge
    /// Locks the corner where anthropic-compat has base_url=None: both the
    /// ANTHROPIC_BASE_URL set AND the beta-disable are gated inside
    /// `if let Some(url)`, so with no URL the token is still set (Bearer) but
    /// betas stay ON and no base URL is written. Mirrors the official-edge
    /// behavior but on the Bearer side.
    #[test]
    fn char_exec_anthropic_compat_no_baseurl_edge() {
        let _g = crate::env_test_guard();
        let _snap = EnvSnapshot::capture(EXECUTOR_ENV_VARS);

        let config = ArisConfig {
            executor_provider: Some("anthropic-compat".into()),
            executor_api_key: Some("K".into()),
            executor_base_url: None,
            ..Default::default()
        };
        config.force_apply_to_env();

        assert_eq!(
            std::env::var("ANTHROPIC_AUTH_TOKEN").ok().as_deref(),
            Some("K")
        );
        // base_url=None → both gated effects skipped.
        assert!(std::env::var("ANTHROPIC_BASE_URL").is_err());
        assert!(
            std::env::var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS").is_err(),
            "betas-disable is gated on Some(url); with None it must stay unset"
        );
    }

    /// case: exec_openai_family
    /// Locks the OpenAI executor path: provider="openai" sets
    /// EXECUTOR_PROVIDER=openai + EXECUTOR_API_KEY + EXECUTOR_BASE_URL, and
    /// writes NO ANTHROPIC_* vars. EXECUTOR_PROVIDER=openai is the exact-match
    /// gate that makes resolve_openai_executor_config return Some.
    #[test]
    fn char_exec_openai_family() {
        let _g = crate::env_test_guard();
        let _snap = EnvSnapshot::capture(EXECUTOR_ENV_VARS);

        let config = ArisConfig {
            executor_provider: Some("openai".into()),
            executor_api_key: Some("K".into()),
            executor_base_url: Some("https://api.openai.com/v1".into()),
            ..Default::default()
        };
        config.force_apply_to_env();

        assert_eq!(
            std::env::var("EXECUTOR_PROVIDER").ok().as_deref(),
            Some("openai")
        );
        assert_eq!(std::env::var("EXECUTOR_API_KEY").ok().as_deref(), Some("K"));
        assert_eq!(
            std::env::var("EXECUTOR_BASE_URL").ok().as_deref(),
            Some("https://api.openai.com/v1")
        );
        // OpenAI path writes no Anthropic vars.
        assert!(std::env::var("ANTHROPIC_API_KEY").is_err());
        assert!(std::env::var("ANTHROPIC_AUTH_TOKEN").is_err());
        assert!(std::env::var("ANTHROPIC_BASE_URL").is_err());
    }

    /// case: exec_custom_maps_to_openai
    /// Locks the custom→openai collapse: provider="custom" is
    /// runtime-indistinguishable from "openai" — it sets EXECUTOR_PROVIDER
    /// to the literal "openai" (NOT "custom") plus EXECUTOR_API_KEY +
    /// EXECUTOR_BASE_URL. (config.json keeps "custom" only for the setup
    /// menu echo; at the env layer it is openai.)
    #[test]
    fn char_exec_custom_maps_to_openai() {
        let _g = crate::env_test_guard();
        let _snap = EnvSnapshot::capture(EXECUTOR_ENV_VARS);

        let config = ArisConfig {
            executor_provider: Some("custom".into()),
            executor_api_key: Some("K".into()),
            executor_base_url: Some("https://proxy.example.com/v1".into()),
            ..Default::default()
        };
        config.force_apply_to_env();

        assert_eq!(
            std::env::var("EXECUTOR_PROVIDER").ok().as_deref(),
            Some("openai"),
            "custom must collapse to literal openai at the env layer"
        );
        assert_eq!(std::env::var("EXECUTOR_API_KEY").ok().as_deref(), Some("K"));
        assert_eq!(
            std::env::var("EXECUTOR_BASE_URL").ok().as_deref(),
            Some("https://proxy.example.com/v1")
        );
        assert!(std::env::var("ANTHROPIC_API_KEY").is_err());
        assert!(std::env::var("ANTHROPIC_AUTH_TOKEN").is_err());
    }

    /// case: force_clears_stale_beta_flag
    /// Companion to the pre-existing force_apply_executor_env test, but via
    /// ForceAll (force_apply_to_env). Locks that a prior run's stale
    /// ANTHROPIC_BASE_URL + beta-disable flag are removed first, so the
    /// official endpoint (base_url=None) runs with betas ON.
    #[test]
    fn char_force_clears_stale_beta_flag_forceall() {
        let _g = crate::env_test_guard();
        let _snap = EnvSnapshot::capture(EXECUTOR_ENV_VARS);

        std::env::set_var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS", "1");
        std::env::set_var("ANTHROPIC_BASE_URL", "https://old-proxy.example.com");

        let config = ArisConfig {
            executor_provider: Some("anthropic".into()),
            executor_api_key: Some("K".into()),
            executor_base_url: None,
            ..Default::default()
        };
        config.force_apply_to_env();

        assert!(std::env::var("ANTHROPIC_BASE_URL").is_err());
        assert!(std::env::var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS").is_err());
        assert_eq!(std::env::var("ANTHROPIC_API_KEY").ok().as_deref(), Some("K"));
    }

    /// case: force_executor_only_preserves_reviewer_keys
    /// Locks the executor/reviewer env isolation under ForceExecutorOnly
    /// (force_apply_executor_env): a shell-provided OPENAI_API_KEY (the
    /// reviewer key) must NOT be cleared when the user re-applies only the
    /// executor auth. force_rev is false in this mode, so the reviewer-clear
    /// block is skipped.
    #[test]
    fn char_force_executor_only_preserves_reviewer_keys() {
        let _g = crate::env_test_guard();
        // Capture executor vars AND OPENAI_API_KEY so we restore both.
        let _snap = EnvSnapshot::capture(EXECUTOR_ENV_VARS);
        let _rev_snap = EnvSnapshot::capture(&["OPENAI_API_KEY"]);

        // Reviewer key supplied by the user's shell.
        std::env::set_var("OPENAI_API_KEY", "reviewer-key");

        let config = ArisConfig {
            executor_provider: Some("anthropic".into()),
            executor_api_key: Some("exec-key".into()),
            executor_base_url: None,
            ..Default::default()
        };
        config.force_apply_executor_env();

        // Executor auth applied …
        assert_eq!(
            std::env::var("ANTHROPIC_API_KEY").ok().as_deref(),
            Some("exec-key")
        );
        // … but the reviewer key survives (ForceExecutorOnly leaves it alone).
        assert_eq!(
            std::env::var("OPENAI_API_KEY").ok().as_deref(),
            Some("reviewer-key"),
            "ForceExecutorOnly must not clobber shell-provided reviewer OPENAI_API_KEY"
        );
    }

    /// case: exec_openai_api_key_fallback (config-layer half)
    /// Locks that the openai-provider env-writing uses EXECUTOR_API_KEY (the
    /// resolver's OPENAI_API_KEY fallback is tested in openai_executor.rs).
    /// Here we pin: a force-apply with provider=openai writes the key to
    /// EXECUTOR_API_KEY, and an IfMissing apply with EXECUTOR_API_KEY already
    /// set leaves the shell value untouched (shell wins).
    #[test]
    fn char_exec_openai_ifmissing_shell_wins() {
        let _g = crate::env_test_guard();
        let _snap = EnvSnapshot::capture(EXECUTOR_ENV_VARS);

        // Shell already provided EXECUTOR_PROVIDER + key.
        std::env::set_var("EXECUTOR_PROVIDER", "openai");
        std::env::set_var("EXECUTOR_API_KEY", "shell-key");

        let config = ArisConfig {
            executor_provider: Some("openai".into()),
            executor_api_key: Some("config-key".into()),
            executor_base_url: Some("https://api.openai.com/v1".into()),
            ..Default::default()
        };
        // IfMissing mode: shell-provided vars win, config does not overwrite.
        config.apply_to_env();

        assert_eq!(
            std::env::var("EXECUTOR_API_KEY").ok().as_deref(),
            Some("shell-key"),
            "IfMissing must not overwrite a shell-provided EXECUTOR_API_KEY"
        );
        // base_url was unset in the shell, so IfMissing fills it from config.
        assert_eq!(
            std::env::var("EXECUTOR_BASE_URL").ok().as_deref(),
            Some("https://api.openai.com/v1")
        );
    }

    // ── setup_menu echo / exec_info mirror tests ──────────────────────────
    //
    // The setup wizard's `exec_info` tuple table and the `default_executor`
    // menu-echo `match` live INLINE inside `run_interactive_setup`, which is
    // interactive (reads stdin). They cannot be unit-tested directly without
    // refactoring production code (out of scope for a characterization
    // agent). The truly load-bearing thing for zero-regression is the RUNTIME
    // env each menu choice produces — so the round-trip tests above already
    // lock the openai/anthropic-compat env contracts those menus map to.
    //
    // The mirror helpers below replicate the production `default_executor`
    // echo `match` VERBATIM so the menu→number routing is pinned. NOTE: these
    // are mirror assertions, not auto-drift detectors — if production changes
    // the table, a reviewer diffing the two catches it; the test itself only
    // fails if the COPIED logic here is edited. They document current echo
    // behavior (DISCREPANCY-aware, see report).

    /// Replica of the production `default_executor` echo match
    /// (config.rs run_interactive_setup, copied verbatim 2026-05-30).
    fn echo_default_executor(provider: Option<&str>, base_url: Option<&str>) -> &'static str {
        match provider {
            Some("anthropic") => "1",
            Some("anthropic-compat") => match base_url {
                Some(u) if u.contains("deepseek") => "7",
                _ => "1",
            },
            Some("custom") => "11",
            Some("openai") => match base_url {
                Some(u) if u.contains("googleapis") => "3",
                Some(u) if u.contains("bigmodel") => "4",
                Some(u) if u.contains("minimax") => "5",
                Some(u) if u.contains("moonshot") => "6",
                Some(u) if u.contains("xiaomimimo") => "8",
                Some(u) if u.contains("dashscope") => "9",
                Some(u) if u.contains("volces") => "10",
                _ => "2",
            },
            _ => "1",
        }
    }

    /// case: setup_menu_3_gemini / 4_glm / 5_minimax / 6_kimi / 7_deepseek /
    /// 8_9_10_echo / 2_or_unknown_proxy_echo — all in one table-driven test.
    /// Locks the executor menu-echo routing (provider + base_url substring →
    /// menu number). Pins each provider's substring keyword and that
    /// anthropic-compat+deepseek echoes "7" while anthropic-compat without a
    /// deepseek URL falls back to "1".
    #[test]
    fn char_setup_menu_default_executor_echo() {
        // (provider, base_url, expected_menu_number)
        let cases: &[(Option<&str>, Option<&str>, &str)] = &[
            // setup_menu_3_gemini: googleapis → "3"
            (
                Some("openai"),
                Some("https://generativelanguage.googleapis.com/v1beta/openai"),
                "3",
            ),
            // setup_menu_4_glm: bigmodel → "4"
            (
                Some("openai"),
                Some("https://open.bigmodel.cn/api/paas/v4"),
                "4",
            ),
            // setup_menu_5_minimax_openai: minimax → "5"
            (Some("openai"), Some("https://api.minimax.chat/v1"), "5"),
            // setup_menu_6_kimi: moonshot → "6"
            (Some("openai"), Some("https://api.moonshot.cn/v1"), "6"),
            // setup_menu_8_9_10_echo: xiaomimimo/dashscope/volces → 8/9/10
            (
                Some("openai"),
                Some("https://token-plan-cn.xiaomimimo.com/v1"),
                "8",
            ),
            (
                Some("openai"),
                Some("https://dashscope.aliyuncs.com/compatible-mode/v1"),
                "9",
            ),
            (
                Some("openai"),
                Some("https://ark.cn-beijing.volces.com/api/v3"),
                "10",
            ),
            // setup_menu_7_deepseek_compat: anthropic-compat + deepseek → "7"
            (
                Some("anthropic-compat"),
                Some("https://api.deepseek.com/anthropic"),
                "7",
            ),
            // anthropic-compat WITHOUT a deepseek URL → falls back to "1".
            (
                Some("anthropic-compat"),
                Some("https://other-compat.example.com/anthropic"),
                "1",
            ),
            // setup_menu_2_or_unknown_proxy_echo: openai + unmatched URL → "2"
            (
                Some("openai"),
                Some("https://my-custom-openai-proxy.example.com/v1"),
                "2",
            ),
            // openai + no URL → "2"
            (Some("openai"), None, "2"),
            // anthropic → "1"; custom → "11"; unknown/None → "1"
            (Some("anthropic"), None, "1"),
            (Some("anthropic"), Some("https://code.newcli.com/claude"), "1"),
            (Some("custom"), None, "11"),
            (None, None, "1"),
        ];
        for (provider, base_url, expected) in cases {
            assert_eq!(
                echo_default_executor(*provider, *base_url),
                *expected,
                "echo mismatch for provider={provider:?} base_url={base_url:?}"
            );
        }
    }
}
