//! In-memory, single-use, expiring approvals binding consent to an exact outbound request.
use crate::error::{AppError, Result};
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const APPROVAL_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Debug)]
pub struct Approval {
    pub session_id: String,
    pub group_id: String,
    /// Exactly what is sent as the user message (shown in the preview).
    pub user_message: String,
    pub allowed_lines: Vec<u64>,
    pub provider_fingerprint: String,
    pub expires: Instant,
    pub retry_used: bool,
    in_flight: bool,
}

impl Approval {
    pub fn new(
        session_id: String,
        group_id: String,
        user_message: String,
        allowed_lines: Vec<u64>,
        provider_fingerprint: String,
        now: Instant,
    ) -> Self {
        Self {
            session_id,
            group_id,
            user_message,
            allowed_lines,
            provider_fingerprint,
            expires: now + APPROVAL_TTL,
            retry_used: false,
            in_flight: false,
        }
    }
}

#[derive(Default)]
pub struct ApprovalStore {
    map: HashMap<String, Approval>,
}

impl ApprovalStore {
    pub fn insert(&mut self, a: Approval, now: Instant) -> String {
        self.map.retain(|_, v| v.expires > now);
        let id = uuid::Uuid::new_v4().to_string();
        self.map.insert(id.clone(), a);
        id
    }

    /// Validate and mark in flight. Fails if missing, expired, bound to another session/provider, or already running.
    pub fn begin(
        &mut self,
        id: &str,
        session_id: &str,
        provider_fp: &str,
        now: Instant,
    ) -> Result<Approval> {
        let a = self.map.get_mut(id).ok_or(AppError::ApprovalInvalid)?;
        let mismatch =
            a.expires <= now || a.session_id != session_id || a.provider_fingerprint != provider_fp;
        if mismatch {
            self.map.remove(id);
            return Err(AppError::ApprovalInvalid);
        }
        if a.in_flight {
            return Err(AppError::ApprovalInvalid);
        }
        a.in_flight = true;
        Ok(a.clone())
    }

    /// Finish a request. `keep_for_retry` keeps it (once) after a retryable failure; otherwise it is consumed.
    /// Returns whether the approval was kept (i.e. one more attempt is possible).
    pub fn finish(&mut self, id: &str, keep_for_retry: bool) -> bool {
        match self.map.get_mut(id) {
            Some(a) if keep_for_retry && !a.retry_used => {
                a.retry_used = true;
                a.in_flight = false;
                true
            }
            _ => {
                self.map.remove(id);
                false
            }
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(now: Instant) -> Approval {
        Approval::new(
            "s1".into(),
            "g1".into(),
            "msg".into(),
            vec![1],
            "p|m".into(),
            now,
        )
    }

    #[test]
    fn single_use() {
        let now = Instant::now();
        let mut st = ApprovalStore::default();
        let id = st.insert(mk(now), now);
        assert!(st.begin(&id, "s1", "p|m", now).is_ok());
        st.finish(&id, false);
        assert_eq!(
            st.begin(&id, "s1", "p|m", now).unwrap_err(),
            AppError::ApprovalInvalid
        );
    }

    #[test]
    fn rejects_other_session_provider_and_expiry() {
        let now = Instant::now();
        let mut st = ApprovalStore::default();
        let id = st.insert(mk(now), now);
        assert!(st.begin(&id, "other", "p|m", now).is_err());
        assert!(st.is_empty(), "mismatch invalidates it");
        let id = st.insert(mk(now), now);
        assert!(st.begin(&id, "s1", "p2|m", now).is_err());
        let id = st.insert(mk(now), now);
        assert!(st.begin(&id, "s1", "p|m", now + APPROVAL_TTL).is_err());
    }

    #[test]
    fn concurrent_use_blocked_and_one_retry_only() {
        let now = Instant::now();
        let mut st = ApprovalStore::default();
        let id = st.insert(mk(now), now);
        st.begin(&id, "s1", "p|m", now).unwrap();
        assert!(
            st.begin(&id, "s1", "p|m", now).is_err(),
            "already in flight"
        );
        st.finish(&id, true); // first failure: keep for the controlled retry
        st.begin(&id, "s1", "p|m", now).unwrap();
        st.finish(&id, true); // second failure: consumed
        assert!(st.begin(&id, "s1", "p|m", now).is_err());
    }
}
