//! State for the AWS SSO session manager (§27.1).
//!
//! Architecture: ARCHITECTURE.md §5.9 (module layout).

use libra_core::{Effect, ProcessId};

use crate::actions::Action;
use crate::model::{
    AppScreen, AuthStatus, ConfigRow, ProcessStep, SavedConfig, DEFAULT_AWS_REGION, DEFAULT_ROLE,
    DEFAULT_SCOPES, DEFAULT_SESSION, DEFAULT_SSO_REGION, DEFAULT_START_URL, STORE_KEY,
    aws_version_cmd,
};

#[derive(Default)]
#[libra::state]
pub struct AwsLogin {
    // ---- preflight (bound) ----
    pub aws_status_line: String,
    pub claude_status_line: String,

    // ---- onboarding form ----
    pub session_name: String,
    pub start_url: String,
    pub sso_region: String,
    pub aws_region: String,
    pub registration_scopes: String,
    pub account_id: String,
    pub role_name: String,
    pub profile_name: String,
    pub onboarding_error: String,
    pub url_fragment_warning: String,

    // ---- steady state ----
    pub identity_line: String,
    pub verified_ago: String,
    pub next_refresh: String,
    pub error_message: String,
    pub auto_refresh: bool,
    pub config_rows: Vec<ConfigRow>,

    // ---- projections (§6.3) ----
    pub is_active: bool,
    pub has_error: bool,
    pub is_busy: bool,
    pub is_onboarding: bool,
    pub is_preflight: bool,
    pub is_steady: bool,
    pub aws_missing: bool,
    pub preflight_busy: bool,
    pub has_url_warning: bool,
    pub has_onboarding_error: bool,

    // ---- internal ----
    #[libra(skip)]
    #[serde(skip, default)]
    pub screen: AppScreen,
    #[libra(skip)]
    #[serde(skip, default)]
    pub auth: AuthStatus,
    #[libra(skip)]
    #[serde(skip, default)]
    pub aws_ok: bool,
    #[libra(skip)]
    #[serde(skip, default)]
    pub process_step: ProcessStep,
    #[libra(skip)]
    #[serde(skip, default)]
    pub config_writes: Vec<(String, String)>,
    #[libra(skip)]
    #[serde(skip, default)]
    pub config_writes_are_profile: bool,
    #[libra(skip)]
    #[serde(skip, default)]
    pub configured: bool,
    #[libra(skip)]
    #[serde(skip, default)]
    pub now_t_ms: f64,
    #[libra(skip)]
    #[serde(skip, default)]
    pub last_login_t_ms: f64,
    #[libra(skip)]
    #[serde(skip, default)]
    pub last_probe_t_ms: f64,
    #[libra(skip)]
    #[serde(skip, default)]
    pub last_verified_t_ms: f64,
    #[libra(skip)]
    #[serde(skip, default)]
    pub active_process: Option<ProcessId>,
    #[libra(skip)]
    #[serde(skip, default)]
    pub next_process_id: u64,
    #[libra(skip)]
    #[serde(skip, default)]
    pub saved: Option<SavedConfig>,
    #[libra(skip)]
    #[serde(skip, default)]
    pub pending_login_after_config: bool,
    #[libra(skip)]
    #[serde(skip, default)]
    pub list_profiles_for_validation: bool,
    #[libra(skip)]
    #[serde(skip, default)]
    pub known_profiles: Vec<String>,
    #[libra(skip)]
    #[serde(skip, default)]
    pub token_cache_paths: Vec<String>,
    #[libra(skip)]
    #[serde(skip, default)]
    pub token_cache_index: usize,
}

impl AwsLogin {
    pub fn apply_form_defaults(&mut self) {
        self.session_name = DEFAULT_SESSION.into();
        self.start_url = DEFAULT_START_URL.into();
        self.sso_region = DEFAULT_SSO_REGION.into();
        self.aws_region = DEFAULT_AWS_REGION.into();
        self.registration_scopes = DEFAULT_SCOPES.into();
        self.account_id.clear();
        self.role_name = DEFAULT_ROLE.into();
        self.profile_name = DEFAULT_ROLE.into();
        self.auto_refresh = true;
    }

    pub fn alloc_process_id(&mut self) -> ProcessId {
        let id = self.next_process_id;
        self.next_process_id += 1;
        ProcessId(id)
    }

    pub fn spawn_process(&mut self, command: libra_core::CommandSpec) -> Effect<Action> {
        let id = self.alloc_process_id();
        self.active_process = Some(id);
        Effect::process_id(id, command, Action::ProcessDone)
    }

    pub fn init() -> (AwsLogin, Vec<Effect<Action>>) {
        let mut state = AwsLogin::default();
        state.apply_form_defaults();
        state.screen = AppScreen::Preflight;
        state.process_step = ProcessStep::PreflightAws;
        state.aws_status_line = "Checking for AWS CLI v2…".into();
        state.claude_status_line = "Claude CLI: not checked yet".into();
        state.__libra_project();
        let fx = state.spawn_process(aws_version_cmd());
        (state, vec![fx])
    }

    pub fn load_config_effect(&mut self) -> Effect<Action> {
        self.process_step = ProcessStep::LoadConfig;
        Effect::kv_get(STORE_KEY, Action::ConfigLoaded)
    }
}
