//! Actions for the AWS SSO session manager (§27.1).
//!
//! Architecture: ARCHITECTURE.md §5.9, §12.6 (process effect).

use libra_core::{ClipboardResult, Effect, Instant, KvResult, ProcessResult, ProcessStatus};

use crate::model::{
    aws_caller_identity, aws_configure_get, aws_configure_set, aws_list_profiles, aws_sso_login,
    claude_version_cmd, config_read_key_list, config_write_pairs, configure_sso_command,
    export_profile_line, format_duration_ago, format_next_refresh, parse_aws_major_version,
    parse_caller_identity, url_fragment_warning, validate_onboarding, AppScreen, AuthStatus,
    ConfigRow, ProcessStep, SavedConfig, PROBE_INTERVAL_MS, REFRESH_INTERVAL_MS, STORE_KEY,
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
        self.profile_name = self.profile_name.trim().to_string();
        if self.profile_name.is_empty() {
            self.profile_name = crate::model::default_profile_name(&self.role_name, &self.account_id);
        }
        self.url_fragment_warning = url_fragment_warning(&self.start_url);
        self.list_profiles_for_validation = true;
        self.process_step = ProcessStep::Idle;
        vec![self.spawn_process(aws_list_profiles())]
    }

    pub fn toggle_config(&mut self) -> Vec<Effect<Action>> {
        self.show_config = !self.show_config;
        if !self.show_config {
            return vec![];
        }
        self.refresh_config_rows_from_saved();
        self.start_config_read()
    }

    pub fn toggle_auto_refresh(&mut self) -> Option<Effect<Action>> {
        self.auto_refresh = !self.auto_refresh;
        self.refresh_time_labels();
        if let Some(saved) = self.saved.clone() {
            let mut updated = saved;
            updated.auto_refresh = self.auto_refresh;
            self.saved = Some(updated.clone());
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
        self.show_config = false;
        self.error_message.clear();
        self.onboarding_error.clear();
        self.apply_form_defaults();
    }

    pub fn session_name_changed(&mut self, text: String) {
        self.session_name = text;
    }
    pub fn start_url_changed(&mut self, text: String) {
        self.start_url = text;
        self.url_fragment_warning = url_fragment_warning(&self.start_url);
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
    pub fn account_id_changed(&mut self, text: String) {
        self.account_id = text;
        self.profile_name = crate::model::default_profile_name(&self.role_name, &self.account_id);
    }
    pub fn role_name_changed(&mut self, text: String) {
        self.role_name = text;
        self.profile_name = crate::model::default_profile_name(&self.role_name, &self.account_id);
    }
    pub fn profile_name_changed(&mut self, text: String) {
        self.profile_name = text;
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
            ProcessStep::Login => self.handle_login(result),
            ProcessStep::Verify => self.handle_verify(result),
            ProcessStep::Probe => self.handle_probe(result),
            ProcessStep::ReadConfigKeys => self.handle_read_config_key(result),
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
        self.has_url_warning = !self.url_fragment_warning.is_empty();
        self.has_onboarding_error = !self.onboarding_error.is_empty();
        self.refresh_time_labels();
    }
}

impl AwsLogin {
    fn profile_name(&self) -> String {
        if let Some(saved) = &self.saved {
            return saved.profile_name.clone();
        }
        if !self.profile_name.is_empty() {
            return self.profile_name.clone();
        }
        crate::model::default_profile_name(&self.role_name, &self.account_id)
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
    }

    fn refresh_time_labels(&mut self) {
        self.verified_ago = format_duration_ago(self.now_t_ms, self.last_verified_t_ms);
        self.next_refresh =
            format_next_refresh(self.now_t_ms, self.last_login_t_ms, self.auto_refresh);
    }

    fn refresh_config_rows_from_saved(&mut self) {
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
            self.config_read_queue = config_read_key_list(cfg);
            self.config_read_index = 0;
            self.process_step = ProcessStep::ReadConfigKeys;
        }
        self.config_rows = rows;
    }

    fn start_config_read(&mut self) -> Vec<Effect<Action>> {
        if self.config_read_index < self.config_read_queue.len() {
            let key = self.config_read_queue[self.config_read_index].clone();
            return vec![self.spawn_process(aws_configure_get(&key))];
        }
        vec![]
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
        if let Err(msg) = validate_onboarding(
            &self.account_id,
            &self.start_url,
            &self.profile_name,
            &profiles,
        ) {
            self.onboarding_error = msg;
            return vec![];
        }
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
        self.saved = Some(cfg.clone());
        self.config_writes = config_write_pairs(&cfg);
        self.process_step = ProcessStep::ConfigWrite;
        self.pending_login_after_config = true;
        self.run_next_config_write()
    }

    fn handle_config_write(&mut self, result: ProcessResult) -> Vec<Effect<Action>> {
        if !exit_ok(&result) {
            self.onboarding_error = stderr_message(&result, "aws configure set failed");
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
            return vec![self.spawn_process(aws_configure_set(&key, &value))];
        }
        self.process_step = ProcessStep::Idle;
        let bytes = serde_json::to_vec(self.saved.as_ref().unwrap()).unwrap_or_default();
        vec![Effect::kv_set_acked(STORE_KEY, bytes, Action::ConfigSaved)]
    }

    fn handle_login(&mut self, result: ProcessResult) -> Vec<Effect<Action>> {
        if !exit_ok(&result) {
            self.auth = AuthStatus::Failed;
            self.error_message = stderr_message(&result, "aws sso login failed");
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
            self.error_message = stderr_message(&result, "identity verification failed");
            self.process_step = ProcessStep::Idle;
            return vec![];
        }
        if let Some((account, arn)) = parse_caller_identity(&result.stdout) {
            self.auth = AuthStatus::Active;
            self.identity_line = format!("{account} · {arn}");
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

    fn handle_read_config_key(&mut self, result: ProcessResult) -> Vec<Effect<Action>> {
        if self.config_read_index < self.config_read_queue.len() {
            let key = self.config_read_queue[self.config_read_index].clone();
            let value = if exit_ok(&result) {
                String::from_utf8_lossy(&result.stdout).trim().to_string()
            } else {
                "(unavailable)".into()
            };
            self.config_rows.push(ConfigRow::new(key, value));
            self.config_read_index += 1;
        }
        if self.config_read_index < self.config_read_queue.len() {
            let key = self.config_read_queue[self.config_read_index].clone();
            return vec![self.spawn_process(aws_configure_get(&key))];
        }
        self.process_step = ProcessStep::Idle;
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
