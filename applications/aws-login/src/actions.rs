//! Actions for the AWS SSO session manager (§27.1).
//!
//! Architecture: ARCHITECTURE.md §5.9, §12.6 (process effect).

use libra_core::{ClipboardResult, Effect, Instant, KvResult, ProcessResult, ProcessStatus};

use crate::model::{
    aws_caller_identity, aws_configure_set, aws_configure_sso_session_set, aws_list_profiles, aws_sso_list_accounts,
    aws_sso_login, aws_sso_login_session, aws_version_cmd, claude_version_cmd,
    account_id_from_profile, config_profile_write_pairs, config_session_write_pairs,
    config_write_pairs, configure_sso_command, export_profile_line, find_profile_for_role,
    format_duration_ago, format_next_refresh, parse_aws_major_version, parse_caller_identity,
    parse_sso_access_token, parse_sso_account_ids, read_file_cmd, sso_config_conflict_hint,
    sso_token_cache_paths, strip_url_fragment, url_fragment_warning, validate_onboarding,
    AppScreen, AuthStatus, ConfigRow, ProcessStep, SavedConfig, PROBE_INTERVAL_MS,
    REFRESH_INTERVAL_MS, STORE_KEY,
};
use crate::state::AwsLogin;

#[libra::actions]
impl AwsLogin {
    pub fn tick(&mut self, instant: Instant) -> Option<Effect<Action>> {
        self.now_t_ms = instant.t_ms;
        self.refresh_time_labels();
        if self.screen != AppScreen::Steady || !self.configured {
            return None;
        }
        if self.auth == AuthStatus::LoggingIn
            || self.auth == AuthStatus::Verifying
            || self.process_step != ProcessStep::Idle
        {
            return None;
        }
        let due_probe = self.now_t_ms - self.last_probe_t_ms >= PROBE_INTERVAL_MS as f64;
        let due_refresh = self.auto_refresh
            && self.last_login_t_ms > 0.0
            && self.now_t_ms - self.last_login_t_ms >= REFRESH_INTERVAL_MS as f64;
        if due_refresh {
            self.begin_login();
            return Some(self.spawn_process(aws_sso_login(&self.profile_name())));
        }
        if due_probe {
            self.process_step = ProcessStep::Probe;
            self.last_probe_t_ms = self.now_t_ms;
            return Some(self.spawn_process(aws_caller_identity(&self.profile_name())));
        }
        None
    }

    pub fn authenticate(&mut self) -> Effect<Action> {
        self.error_message.clear();
        self.begin_login();
        self.spawn_process(aws_sso_login(&self.profile_name()))
    }

    pub fn cancel(&mut self) -> Vec<Effect<Action>> {
        let Some(id) = self.active_process.take() else {
            return vec![];
        };
        self.auth = AuthStatus::LoggedOut;
        self.process_step = ProcessStep::Idle;
        vec![Effect::CancelProcess { id }]
    }

    pub fn submit_onboarding(&mut self) -> Vec<Effect<Action>> {
        self.onboarding_error.clear();
        self.session_name = self.session_name.trim().to_string();
        self.start_url = strip_url_fragment(&self.start_url);
        self.url_fragment_warning = url_fragment_warning(&self.start_url);
        self.sync_profile_name();
        self.known_profiles.clear();
        self.list_profiles_for_validation = true;
        self.process_step = ProcessStep::Idle;
        vec![self.spawn_process(aws_list_profiles())]
    }

    pub fn toggle_auto_refresh(&mut self) -> Option<Effect<Action>> {
        self.auto_refresh = !self.auto_refresh;
        self.refresh_time_labels();
        if let Some(saved) = self.saved.clone() {
            let mut updated = saved;
            updated.auto_refresh = self.auto_refresh;
            self.saved = Some(updated.clone());
            self.populate_config_rows_from_saved();
            let bytes = serde_json::to_vec(&updated).unwrap_or_default();
            return Some(Effect::kv_set(STORE_KEY, bytes));
        }
        None
    }

    pub fn copy_export_line(&mut self) -> Effect<Action> {
        Effect::clipboard_set_acked(export_profile_line(&self.profile_name()), Action::ClipboardDone)
    }

    pub fn copy_configure_sso(&mut self) -> Effect<Action> {
        Effect::clipboard_set_acked(configure_sso_command(), Action::ClipboardDone)
    }

    pub fn restart_onboarding(&mut self) {
        self.screen = AppScreen::Onboarding;
        self.auth = AuthStatus::Unknown;
        self.configured = false;
        self.config_rows.clear();
        self.error_message.clear();
        self.onboarding_error.clear();
        self.known_profiles.clear();
        self.apply_form_defaults();
    }

    pub fn retry_preflight(&mut self) -> Effect<Action> {
        self.screen = AppScreen::Preflight;
        self.aws_ok = false;
        self.aws_status_line = "Checking for AWS CLI v2…".into();
        self.claude_status_line = "Claude CLI: not checked yet".into();
        self.process_step = ProcessStep::PreflightAws;
        self.spawn_process(aws_version_cmd())
    }

    pub fn start_url_changed(&mut self, text: String) {
        self.start_url = text;
        self.url_fragment_warning = url_fragment_warning(&self.start_url);
    }
    pub fn session_name_changed(&mut self, text: String) {
        self.session_name = text;
    }
    pub fn sso_region_changed(&mut self, text: String) {
        self.sso_region = text;
    }
    pub fn aws_region_changed(&mut self, text: String) {
        self.aws_region = text;
    }
    pub fn registration_scopes_changed(&mut self, text: String) {
        self.registration_scopes = text;
    }
    pub fn role_name_changed(&mut self, text: String) {
        self.role_name = text;
        self.sync_profile_name();
    }

    pub fn process_done(&mut self, result: ProcessResult) -> Vec<Effect<Action>> {
        self.active_process = None;
        if result.status == ProcessStatus::Signalled(9) {
            self.process_step = ProcessStep::Idle;
            self.auth = AuthStatus::LoggedOut;
            self.error_message = "Login cancelled".into();
            return vec![];
        }
        match self.process_step {
            ProcessStep::PreflightAws => self.handle_preflight_aws(result),
            ProcessStep::PreflightClaude => self.handle_preflight_claude(result),
            ProcessStep::LoadConfig => vec![],
            ProcessStep::ConfigWrite => self.handle_config_write(result),
            ProcessStep::LoginSession => self.handle_login_session(result),
            ProcessStep::ReadAccessToken => self.handle_read_access_token(result),
            ProcessStep::ListAccounts => self.handle_list_accounts(result),
            ProcessStep::Login => self.handle_login(result),
            ProcessStep::Verify => self.handle_verify(result),
            ProcessStep::Probe => self.handle_probe(result),
            ProcessStep::Idle if self.list_profiles_for_validation => {
                self.handle_list_profiles_for_validation(result)
            }
            ProcessStep::Idle => vec![],
        }
    }

    pub fn config_loaded(&mut self, bytes: Option<Vec<u8>>) -> Vec<Effect<Action>> {
        self.process_step = ProcessStep::Idle;
        if let Some(raw) = bytes {
            if let Ok(cfg) = serde_json::from_slice::<SavedConfig>(&raw) {
                self.apply_saved_config(cfg);
                self.screen = AppScreen::Steady;
                self.configured = true;
                self.auth = AuthStatus::Unknown;
                self.process_step = ProcessStep::Probe;
                self.last_probe_t_ms = self.now_t_ms;
                return vec![self.spawn_process(aws_caller_identity(&self.profile_name()))];
            }
        }
        self.screen = AppScreen::Onboarding;
        self.configured = false;
        vec![]
    }

    pub fn config_saved(&mut self, result: KvResult) -> Vec<Effect<Action>> {
        if let KvResult::Err(msg) = result {
            self.onboarding_error = format!("could not save config: {msg}");
            return vec![];
        }
        if self.pending_login_after_config {
            self.pending_login_after_config = false;
            self.screen = AppScreen::Steady;
            self.configured = true;
            self.begin_login();
            return vec![self.spawn_process(aws_sso_login(&self.profile_name()))];
        }
        vec![]
    }

    pub fn clipboard_done(&mut self, _result: ClipboardResult) {}

    #[project]
    fn project(&mut self) {
        self.is_preflight = self.screen == AppScreen::Preflight;
        self.is_onboarding = self.screen == AppScreen::Onboarding;
        self.is_steady = self.screen == AppScreen::Steady;
        self.is_active = self.auth == AuthStatus::Active;
        self.has_error = !self.error_message.is_empty();
        self.is_busy = matches!(
            self.auth,
            AuthStatus::LoggingIn | AuthStatus::Verifying
        ) || self.active_process.is_some();
        self.aws_missing = self.screen == AppScreen::Preflight && !self.aws_ok;
        self.preflight_busy =
            self.screen == AppScreen::Preflight && self.active_process.is_some();
        self.has_url_warning = !self.url_fragment_warning.is_empty();
        self.has_onboarding_error = !self.onboarding_error.is_empty();
        self.refresh_time_labels();
    }
}

impl AwsLogin {
    fn sync_profile_name(&mut self) {
        let role = self.role_name.trim();
        if role.is_empty() {
            self.profile_name.clear();
            return;
        }
        self.profile_name = if self.account_id.is_empty() {
            role.to_string()
        } else {
            crate::model::default_profile_name(role, &self.account_id)
        };
    }

    fn profile_name(&self) -> String {
        if let Some(saved) = &self.saved {
            return saved.profile_name.clone();
        }
        if !self.profile_name.is_empty() {
            return self.profile_name.clone();
        }
        crate::model::default_profile_name(&self.role_name, &self.account_id)
    }

    /// `stderr_message` plus a hint when AWS CLI reports a legacy per-profile
    /// `sso_start_url` conflicting with the linked `sso-session` (the CLI
    /// never auto-heals this — see `sso_config_conflict_hint`).
    fn profile_error_message(&self, result: &ProcessResult, prefix: &str) -> String {
        let base = stderr_message(result, prefix);
        match sso_config_conflict_hint(&base, &self.profile_name()) {
            Some(hint) => format!("{base}\n\n{hint}"),
            None => base,
        }
    }

    fn begin_login(&mut self) {
        self.auth = AuthStatus::LoggingIn;
        self.error_message.clear();
        self.process_step = ProcessStep::Login;
    }

    fn apply_saved_config(&mut self, cfg: SavedConfig) {
        self.session_name = cfg.session_name.clone();
        self.start_url = cfg.start_url.clone();
        self.sso_region = cfg.sso_region.clone();
        self.aws_region = cfg.aws_region.clone();
        self.registration_scopes = cfg.registration_scopes.clone();
        self.account_id = cfg.account_id.clone();
        self.role_name = cfg.role_name.clone();
        self.profile_name = cfg.profile_name.clone();
        self.auto_refresh = cfg.auto_refresh;
        self.saved = Some(cfg);
        self.populate_config_rows_from_saved();
    }

    fn refresh_time_labels(&mut self) {
        self.verified_ago = format_duration_ago(self.now_t_ms, self.last_verified_t_ms);
        self.next_refresh =
            format_next_refresh(self.now_t_ms, self.last_login_t_ms, self.auto_refresh);
    }

    fn populate_config_rows_from_saved(&mut self) {
        let mut rows = vec![];
        if let Some(cfg) = &self.saved {
            rows.push(ConfigRow::new("session", cfg.session_name.clone()));
            rows.push(ConfigRow::new("profile", cfg.profile_name.clone()));
            rows.push(ConfigRow::new("account", cfg.account_id.clone()));
            rows.push(ConfigRow::new("role", cfg.role_name.clone()));
            rows.push(ConfigRow::new("sso region", cfg.sso_region.clone()));
            rows.push(ConfigRow::new("aws region", cfg.aws_region.clone()));
            rows.push(ConfigRow::new(
                "auto refresh",
                if cfg.auto_refresh { "on" } else { "off" },
            ));
            for (key, value) in config_write_pairs(cfg) {
                rows.push(ConfigRow::new(key, value));
            }
        }
        self.config_rows = rows;
    }

    fn handle_preflight_aws(&mut self, result: ProcessResult) -> Vec<Effect<Action>> {
        self.aws_ok = exit_ok(&result);
        if self.aws_ok {
            if let Some(major) = parse_aws_major_version(&result.stdout) {
                if major >= 2 {
                    self.aws_ok = true;
                    self.aws_status_line = String::from_utf8_lossy(&result.stdout).trim().into();
                    self.process_step = ProcessStep::PreflightClaude;
                    self.claude_status_line = "Checking for Claude CLI…".into();
                    return vec![self.spawn_process(claude_version_cmd())];
                }
                self.aws_ok = false;
                self.aws_status_line = format!(
                    "AWS CLI v{major} found — v2 is required. Install from Company Portal or AWS docs."
                );
            } else {
                self.aws_ok = false;
                self.aws_status_line =
                    "Could not parse aws --version output. Install AWS CLI v2.".into();
            }
        } else {
            self.aws_status_line = preflight_failure_message(
                "AWS CLI v2",
                &result,
                "Install AWS CLI v2 from Company Portal or https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html",
            );
        }
        self.screen = AppScreen::Preflight;
        self.process_step = ProcessStep::Idle;
        vec![]
    }

    fn handle_preflight_claude(&mut self, result: ProcessResult) -> Vec<Effect<Action>> {
        if exit_ok(&result) {
            self.claude_status_line =
                format!("Claude CLI: {}", String::from_utf8_lossy(&result.stdout).trim());
        } else {
            self.claude_status_line = "Claude CLI: not installed (optional — for Claude Code setup)".into();
        }
        self.process_step = ProcessStep::LoadConfig;
        vec![self.load_config_effect()]
    }

    fn handle_list_profiles_for_validation(&mut self, result: ProcessResult) -> Vec<Effect<Action>> {
        self.list_profiles_for_validation = false;
        let profiles: Vec<String> = if exit_ok(&result) {
            String::from_utf8_lossy(&result.stdout)
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect()
        } else {
            vec![]
        };
        self.known_profiles = profiles;
        if let Err(msg) =
            validate_onboarding(&self.session_name, &self.start_url, &self.role_name)
        {
            self.onboarding_error = msg;
            return vec![];
        }
        self.sync_profile_name();
        let cfg = SavedConfig::from_form(
            &self.session_name,
            &self.start_url,
            &self.sso_region,
            &self.aws_region,
            &self.registration_scopes,
            &self.account_id,
            &self.role_name,
            &self.profile_name,
            self.auto_refresh,
        );
        self.saved = Some(cfg);
        self.pending_login_after_config = true;

        self.config_writes_are_profile = false;
        self.config_writes = config_session_write_pairs(
            &self.session_name,
            &self.start_url,
            &self.sso_region,
            &self.registration_scopes,
        );
        self.process_step = ProcessStep::ConfigWrite;
        self.run_next_config_write()
    }

    fn handle_config_write(&mut self, result: ProcessResult) -> Vec<Effect<Action>> {
        if !exit_ok(&result) {
            self.onboarding_error = stderr_message(
                &result,
                "aws configure set failed (SSO sessions need AWS CLI v2.34.22 or newer)",
            );
            self.process_step = ProcessStep::Idle;
            self.pending_login_after_config = false;
            return vec![];
        }
        self.run_next_config_write()
    }

    fn run_next_config_write(&mut self) -> Vec<Effect<Action>> {
        if let Some((key, value)) = self.config_writes.first().cloned() {
            self.config_writes.remove(0);
            self.process_step = ProcessStep::ConfigWrite;
            let command = if self.config_writes_are_profile {
                aws_configure_set(&key, &value)
            } else {
                aws_configure_sso_session_set(&self.session_name, &key, &value)
            };
            return vec![self.spawn_process(command)];
        }
        if !self.config_writes_are_profile {
            self.process_step = ProcessStep::LoginSession;
            return vec![self.spawn_process(aws_sso_login_session(&self.session_name))];
        }
        self.process_step = ProcessStep::Idle;
        let bytes = serde_json::to_vec(self.saved.as_ref().unwrap()).unwrap_or_default();
        vec![Effect::kv_set_acked(STORE_KEY, bytes, Action::ConfigSaved)]
    }

    fn handle_login_session(&mut self, result: ProcessResult) -> Vec<Effect<Action>> {
        if !exit_ok(&result) {
            self.onboarding_error = stderr_message(&result, "SSO session login failed");
            self.process_step = ProcessStep::Idle;
            self.pending_login_after_config = false;
            return vec![];
        }
        // Re-setup with an existing profile (refresh_aws_sso.sh parity): derive the
        // account from the profile name instead of listing accounts via OIDC token.
        if let Some(profile) = find_profile_for_role(&self.role_name, &self.known_profiles) {
            if let Some(account_id) = account_id_from_profile(&profile) {
                self.profile_name = profile;
                return self.finish_onboarding_with_account(account_id);
            }
        }
        self.process_step = ProcessStep::ReadAccessToken;
        self.begin_read_access_token()
    }

    fn begin_read_access_token(&mut self) -> Vec<Effect<Action>> {
        self.token_cache_paths = sso_token_cache_paths(&self.session_name, &self.start_url)
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        self.token_cache_index = 0;
        if self.token_cache_paths.is_empty() {
            self.onboarding_error = "could not locate AWS config directory".into();
            self.process_step = ProcessStep::Idle;
            self.pending_login_after_config = false;
            return vec![];
        }
        vec![self.spawn_process(read_file_cmd(&self.token_cache_paths[0]))]
    }

    fn handle_read_access_token(&mut self, result: ProcessResult) -> Vec<Effect<Action>> {
        if !exit_ok(&result) {
            self.token_cache_index += 1;
            if self.token_cache_index < self.token_cache_paths.len() {
                let path = self.token_cache_paths[self.token_cache_index].clone();
                return vec![self.spawn_process(read_file_cmd(&path))];
            }
            self.onboarding_error =
                stderr_message(&result, "could not read cached SSO access token");
            self.process_step = ProcessStep::Idle;
            self.pending_login_after_config = false;
            return vec![];
        }
        let token = match parse_sso_access_token(&result.stdout) {
            Ok(t) => t,
            Err(msg) => {
                self.onboarding_error = msg;
                self.process_step = ProcessStep::Idle;
                self.pending_login_after_config = false;
                return vec![];
            }
        };
        self.process_step = ProcessStep::ListAccounts;
        vec![self.spawn_process(aws_sso_list_accounts(&token, &self.sso_region))]
    }

    fn handle_list_accounts(&mut self, result: ProcessResult) -> Vec<Effect<Action>> {
        if !exit_ok(&result) {
            self.onboarding_error = stderr_message(&result, "could not list AWS accounts");
            self.process_step = ProcessStep::Idle;
            self.pending_login_after_config = false;
            return vec![];
        }
        let accounts = match parse_sso_account_ids(&result.stdout) {
            Ok(ids) => ids,
            Err(msg) => {
                self.onboarding_error = msg;
                self.process_step = ProcessStep::Idle;
                self.pending_login_after_config = false;
                return vec![];
            }
        };
        self.finish_onboarding_with_account(accounts[0].clone())
    }

    fn finish_onboarding_with_account(&mut self, account_id: String) -> Vec<Effect<Action>> {
        self.account_id = account_id.clone();
        self.sync_profile_name();
        let cfg = SavedConfig::from_form(
            &self.session_name,
            &self.start_url,
            &self.sso_region,
            &self.aws_region,
            &self.registration_scopes,
            &account_id,
            &self.role_name,
            &self.profile_name,
            self.auto_refresh,
        );
        self.saved = Some(cfg.clone());

        // Match refresh_aws_sso.sh ensure_sso_configured: an existing profile is
        // reused as-is — no aws configure set pass, just save app prefs and sign in.
        if self.known_profiles.iter().any(|p| p == &self.profile_name) {
            self.config_writes.clear();
            self.config_writes_are_profile = false;
            self.process_step = ProcessStep::Idle;
            let bytes = serde_json::to_vec(self.saved.as_ref().unwrap()).unwrap_or_default();
            return vec![Effect::kv_set_acked(STORE_KEY, bytes, Action::ConfigSaved)];
        }

        self.config_writes_are_profile = true;
        self.config_writes = config_profile_write_pairs(&cfg);
        self.process_step = ProcessStep::ConfigWrite;
        self.run_next_config_write()
    }

    fn handle_login(&mut self, result: ProcessResult) -> Vec<Effect<Action>> {
        if !exit_ok(&result) {
            self.auth = AuthStatus::Failed;
            self.error_message = self.profile_error_message(&result, "aws sso login failed");
            self.process_step = ProcessStep::Idle;
            return vec![];
        }
        self.auth = AuthStatus::Verifying;
        self.process_step = ProcessStep::Verify;
        vec![self.spawn_process(aws_caller_identity(&self.profile_name()))]
    }

    fn handle_verify(&mut self, result: ProcessResult) -> Vec<Effect<Action>> {
        if !exit_ok(&result) {
            self.auth = AuthStatus::Failed;
            self.error_message = self.profile_error_message(&result, "identity verification failed");
            self.process_step = ProcessStep::Idle;
            return vec![];
        }
        if let Some((account, arn)) = parse_caller_identity(&result.stdout) {
            self.auth = AuthStatus::Active;
            self.identity_line = format!("{account} · {arn}");
            self.account_id = account.clone();
            if let Some(saved) = self.saved.as_mut() {
                saved.account_id = account;
            }
            self.populate_config_rows_from_saved();
            self.last_login_t_ms = self.now_t_ms;
            self.last_verified_t_ms = self.now_t_ms;
            self.last_probe_t_ms = self.now_t_ms;
            self.error_message.clear();
        } else {
            self.auth = AuthStatus::Failed;
            self.error_message = "could not parse get-caller-identity output".into();
        }
        self.process_step = ProcessStep::Idle;
        vec![]
    }

    fn handle_probe(&mut self, result: ProcessResult) -> Vec<Effect<Action>> {
        self.process_step = ProcessStep::Idle;
        if exit_ok(&result) {
            if let Some((account, arn)) = parse_caller_identity(&result.stdout) {
                self.auth = AuthStatus::Active;
                self.identity_line = format!("{account} · {arn}");
                self.account_id = account.clone();
                if let Some(saved) = self.saved.as_mut() {
                    saved.account_id = account;
                }
                self.populate_config_rows_from_saved();
                self.last_verified_t_ms = self.now_t_ms;
                self.error_message.clear();
                return vec![];
            }
        }
        self.auth = AuthStatus::LoggedOut;
        self.identity_line = "Not authenticated".into();
        if self.auto_refresh && self.screen == AppScreen::Steady {
            self.begin_login();
            return vec![self.spawn_process(aws_sso_login(&self.profile_name()))];
        }
        vec![]
    }
}

fn exit_ok(result: &ProcessResult) -> bool {
    matches!(result.status, ProcessStatus::Exited(0))
}

fn stderr_message(result: &ProcessResult, prefix: &str) -> String {
    let detail = String::from_utf8_lossy(&result.stderr).trim().to_string();
    if detail.is_empty() {
        prefix.into()
    } else {
        format!("{prefix}: {detail}")
    }
}

fn preflight_failure_message(tool: &str, result: &ProcessResult, hint: &str) -> String {
    match &result.status {
        ProcessStatus::SpawnFailed(msg) => format!("{tool} not found ({msg}). {hint}"),
        _ => format!("{tool} not available. {hint}"),
    }
}
