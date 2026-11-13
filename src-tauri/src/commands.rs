use crate::demo::{self, DemoAnalysis};
use crate::state::{AppState, CmdError, CmdResult};
use serde::Serialize;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use wotslog_core::AppError;
use wotslog_core::ai::{self, SYSTEM_PROMPT};
use wotslog_core::approval::{APPROVAL_TTL, Approval};
use wotslog_core::credentials::set_verified;
use wotslog_core::excerpt::build_excerpt;
use wotslog_core::scan::{MAX_FILE_BYTES, scan_file};
use wotslog_core::search::{QuerySpec, query_page};
use wotslog_core::session::{Session, SessionInfo, Source};
use wotslog_core::settings::{self, ProviderSettings};
use wotslog_core::types::{Diagnosis, Page, Severity, Summary};

const ALLOWED_EXT: [&str; 5] = ["log", "txt", "ndjson", "jsonl", "json"];

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Io(e.to_string()))?
        .map_err(CmdError::from)
}

#[tauri::command]
pub async fn select_log(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Option<SessionInfo>> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Open a log file")
            .add_filter("Log files", &ALLOWED_EXT)
            .blocking_pick_file()
    })
    .await
    .map_err(|e| AppError::Io(e.to_string()))?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked
        .into_path()
        .map_err(|e| AppError::Io(e.to_string()))?;
    let ext_ok = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ALLOWED_EXT.contains(&e.to_ascii_lowercase().as_str()));
    if !ext_ok {
        return Err(AppError::InvalidArgument(
            "choose a .log, .txt, .ndjson, .jsonl or .json file".into(),
        )
        .into());
    }
    let session = blocking(move || Session::open(path, Source::Local)).await?;
    let info = session.info();
    state.install_session(session);
    Ok(Some(info))
}

#[tauri::command]
pub async fn load_demo(app: AppHandle, state: State<'_, AppState>) -> CmdResult<SessionInfo> {
    let path = demo::demo_path(&app)?;
    let session = blocking(move || Session::open(path, Source::Demo)).await?;
    let info = session.info();
    state.install_session(session);
    Ok(info)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    scan_id: String,
    bytes_read: u64,
    total_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    session_id: String,
    scan_id: String,
    max_file_bytes: u64,
    #[serde(flatten)]
    summary: Summary,
}

#[tauri::command]
pub async fn scan_log(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    scan_id: String,
) -> CmdResult<ScanResult> {
    let sess = state.current(&session_id)?;
    let flag = state.register_scan(&scan_id);
    let (f2, sid) = (flag.clone(), scan_id.clone());
    let total = std::fs::metadata(&sess.path).map(|m| m.len()).unwrap_or(0);
    let res = blocking(move || {
        scan_file(&sess.path, &f2, true, |b| {
            let _ = app.emit(
                "scan-progress",
                Progress {
                    scan_id: sid.clone(),
                    bytes_read: b,
                    total_bytes: total,
                },
            );
        })
    })
    .await;
    state.finish_scan(&scan_id, &flag);
    let summary = res?;
    // Discard if the user switched files, or the file changed, while we were scanning.
    let mut g = state.session.lock().unwrap();
    let cur = g.as_mut().ok_or(AppError::NoSession)?;
    if cur.id != session_id {
        return Err(AppError::StaleSession.into());
    }
    cur.verify_unchanged()?;
    cur.summary = Some(summary.clone());
    Ok(ScanResult {
        session_id,
        scan_id,
        max_file_bytes: MAX_FILE_BYTES,
        summary,
    })
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>, scan_id: String) {
    state.cancel_scan(&scan_id);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageResult {
    session_id: String,
    scan_id: String,
    #[serde(flatten)]
    page: Page,
}

#[tauri::command]
pub async fn query_logs(
    state: State<'_, AppState>,
    session_id: String,
    query: String,
    severity: Option<String>,
    offset: u64,
    limit: u64,
    scan_id: String,
) -> CmdResult<PageResult> {
    if query.len() > 1024 {
        return Err(
            AppError::InvalidArgument("search text is limited to 1024 bytes".into()).into(),
        );
    }
    let severity = match severity.as_deref() {
        None | Some("") | Some("ALL") => None,
        Some("ERROR") => Some(Severity::Error),
        Some("WARN") => Some(Severity::Warn),
        Some("INFO") => Some(Severity::Info),
        Some("DEBUG") => Some(Severity::Debug),
        Some("UNKNOWN") => Some(Severity::Unknown),
        Some(o) => return Err(AppError::InvalidArgument(format!("unknown severity {o}")).into()),
    };
    let sess = state.current(&session_id)?;
    let flag = state.register_scan(&scan_id);
    let f2 = flag.clone();
    let res = blocking(move || {
        query_page(
            &sess.path,
            &QuerySpec { query, severity },
            offset,
            limit,
            &f2,
        )
    })
    .await;
    state.finish_scan(&scan_id, &flag);
    let page = res?;
    state.current(&session_id)?; // still the same session and unchanged
    Ok(PageResult {
        session_id,
        scan_id,
        page,
    })
}

// ---------------- AI ----------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    base_url: String,
    model: String,
    host: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisPreview {
    approval_id: String,
    group_id: String,
    signature: String,
    line_numbers: Vec<u64>,
    /// The exact text sent as the user message (contains the redacted excerpt).
    outbound_message: String,
    system_prompt: &'static str,
    provider: ProviderView,
    sends_api_key: bool,
    redactions: std::collections::BTreeMap<&'static str, usize>,
    outbound_bytes: usize,
    expires_in_secs: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResult {
    diagnosis: Diagnosis,
    is_demo: bool,
    provider: ProviderView,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettingsView {
    base_url: String,
    model: String,
    has_key: bool,
    configured: bool,
    credential_store_ok: bool,
    live_ai_available: bool,
    live_ai_reason: Option<String>,
}

fn provider_view(s: &ProviderSettings) -> ProviderView {
    let host = ai::validate_base_url(&s.base_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    ProviderView {
        base_url: s.base_url.clone(),
        model: s.model.clone(),
        host,
    }
}

fn settings_view(state: &AppState) -> AiSettingsView {
    let loaded = settings::load(&state.settings_path);
    let s = loaded.clone().unwrap_or_default();
    let (has_key, store_ok) = match state.creds.get() {
        Ok(k) => (k.is_some_and(|k| !k.is_empty()), true),
        Err(_) => (false, false),
    };
    let url_ok = ai::validate_base_url(&s.base_url).is_ok();
    let reason = if loaded.is_none() {
        Some("Not configured. Add a provider in Settings to enable live AI.".to_string())
    } else if !url_ok {
        Some("Provider URL is invalid.".into())
    } else if !store_ok {
        Some("The OS credential store is unavailable, so live AI is disabled.".into())
    } else {
        None
    };
    AiSettingsView {
        base_url: s.base_url,
        model: s.model,
        has_key,
        configured: loaded.is_some(),
        credential_store_ok: store_ok,
        live_ai_available: reason.is_none(),
        live_ai_reason: reason,
    }
}

#[tauri::command]
pub fn get_ai_settings(state: State<'_, AppState>) -> AiSettingsView {
    settings_view(&state)
}

#[tauri::command]
pub fn save_ai_settings(
    state: State<'_, AppState>,
    base_url: String,
    model: String,
    api_key: Option<String>,
    clear_key: bool,
) -> CmdResult<AiSettingsView> {
    ai::validate_base_url(&base_url)?;
    let model = model.trim().to_string();
    if model.is_empty() || model.len() > 200 {
        return Err(
            AppError::InvalidArgument("model name is required (max 200 chars)".into()).into(),
        );
    }
    // Key first: if the OS credential store cannot be trusted, nothing is saved and live AI stays disabled.
    if clear_key {
        state.creds.delete()?;
    } else if let Some(k) = api_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
        set_verified(state.creds.as_ref(), k)?;
    }
    settings::save(
        &state.settings_path,
        &ProviderSettings {
            base_url: base_url.trim().to_string(),
            model,
        },
    )?;
    state.approvals.lock().unwrap().clear(); // provider changed: pending consent no longer applies
    Ok(settings_view(&state))
}

fn require_live(state: &AppState) -> Result<ProviderSettings, AppError> {
    let v = settings_view(state);
    if !v.live_ai_available {
        return Err(AppError::AiDisabled(v.live_ai_reason.unwrap_or_default()));
    }
    settings::load(&state.settings_path).ok_or(AppError::AiNotConfigured)
}

#[tauri::command]
pub async fn prepare_analysis(
    state: State<'_, AppState>,
    session_id: String,
    group_id: String,
) -> CmdResult<AnalysisPreview> {
    let provider = require_live(&state)?;
    let sess = state.current(&session_id)?;
    let group = sess
        .summary
        .as_ref()
        .ok_or(AppError::NotScanned)?
        .top_errors
        .iter()
        .find(|g| g.id == group_id)
        .cloned()
        .ok_or(AppError::GroupNotFound)?;
    let path = sess.path.clone();
    let sig = group.signature.clone();
    let ex = blocking(move || build_excerpt(&path, &sig, &AtomicBool::new(false))).await?;
    state.current(&session_id)?;
    let message = ai::build_user_message(&ex.text);
    let now = Instant::now();
    let approval_id = state.approvals.lock().unwrap().insert(
        Approval::new(
            sess.id.clone(),
            group.id.clone(),
            message.clone(),
            ex.line_numbers.clone(),
            provider.fingerprint(),
            now,
        ),
        now,
    );
    let sends_api_key = state
        .creds
        .get()
        .ok()
        .flatten()
        .is_some_and(|k| !k.is_empty());
    Ok(AnalysisPreview {
        approval_id,
        group_id: group.id,
        signature: group.signature,
        line_numbers: ex.line_numbers,
        outbound_bytes: message.len(),
        outbound_message: message,
        system_prompt: SYSTEM_PROMPT,
        provider: provider_view(&provider),
        sends_api_key,
        redactions: ex.redactions,
        expires_in_secs: APPROVAL_TTL.as_secs(),
    })
}

#[tauri::command]
pub async fn analyze_error(
    state: State<'_, AppState>,
    approval_id: String,
) -> CmdResult<AnalysisResult> {
    let provider = require_live(&state)?;
    let sid = state
        .session
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| s.id.clone())
        .ok_or(AppError::NoSession)?;
    state.current(&sid)?;
    let approval = state.approvals.lock().unwrap().begin(
        &approval_id,
        &sid,
        &provider.fingerprint(),
        Instant::now(),
    )?;
    let key = state.creds.get().ok().flatten();
    let res = ai::request_diagnosis(
        &provider,
        key.as_deref(),
        &approval.user_message,
        &approval.allowed_lines,
        ai::REQUEST_TIMEOUT,
    )
    .await;
    match res {
        Ok(diagnosis) => {
            state.approvals.lock().unwrap().finish(&approval_id, false);
            Ok(AnalysisResult {
                diagnosis,
                is_demo: false,
                provider: provider_view(&provider),
            })
        }
        Err(err) => {
            let retry_allowed = state
                .approvals
                .lock()
                .unwrap()
                .finish(&approval_id, err.is_retryable());
            Err(CmdError { err, retry_allowed })
        }
    }
}

#[tauri::command]
pub fn demo_analysis(
    state: State<'_, AppState>,
    session_id: String,
    group_id: String,
) -> CmdResult<DemoAnalysis> {
    let sess = state.current(&session_id)?;
    if sess.source != Source::Demo {
        return Err(AppError::InvalidArgument(
            "example analyses exist only for the bundled demo log".into(),
        )
        .into());
    }
    let group = sess
        .summary
        .as_ref()
        .ok_or(AppError::NotScanned)?
        .top_errors
        .iter()
        .find(|g| g.id == group_id)
        .ok_or(AppError::GroupNotFound)?;
    Ok(demo::example_for(group)?)
}
