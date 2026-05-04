//! Per-`(addr, creds)` cache of [`OnvifSession`].
//!
//! `oxvif::OnvifSession` is built for reuse: every method takes
//! `&self`, `Clone` is cheap (the underlying transport is `Arc`-ed),
//! and the `Capabilities` snapshot is fetched once at `build()` and
//! never refetched. The application layer's job is simply to *hold
//! one alive* across UI actions instead of rebuilding it each call —
//! every rebuild costs one `GetCapabilities` round-trip plus a fresh
//! TCP/TLS handshake.
//!
//! This module is the showcase for "how an oxvif consumer reuses
//! sessions efficiently": one process-wide pool, keyed by `(addr,
//! creds)`. Hit → cheap `Arc` clone. Miss → build once, cache,
//! return. Cred change or device removal → explicit
//! [`invalidate`]. No TTL, no background eviction — sessions live
//! exactly as long as they're useful and the app has direct
//! control over when to drop them.
//!
//! Empirically: an oxdm session navigating a few tabs roughly halves
//! its SOAP traffic vs the no-cache baseline. The savings come from
//! eliminating the per-call `GetCapabilities` plus reusing the
//! reqwest connection pool.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use oxvif::{OnvifError, OnvifSession};

// ── Process-wide singleton ────────────────────────────────────────────────

static POOL: OnceLock<SessionPool> = OnceLock::new();

fn pool() -> &'static SessionPool {
    POOL.get_or_init(SessionPool::new)
}

/// Get-or-build the cached [`OnvifSession`] for `(addr, creds)`.
///
/// Returns an `Arc` so callers can share the session freely without
/// copying the underlying state. Subsequent calls with the same
/// arguments return the same `Arc` (cheap clone, no I/O).
pub async fn get(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Arc<OnvifSession>, OnvifError> {
    pool().get(addr, username, password).await
}

/// Drop every cached session for `addr`.
///
/// Call this when the device's credentials change (so the next API
/// call rebuilds the session under the new creds), or when the
/// device is removed from the app.
pub fn invalidate(addr: &str) {
    pool().invalidate(addr);
}

/// Drop every cached session.
///
/// Used when global credentials change (which can affect any device
/// without per-device overrides) or at app teardown.
pub fn invalidate_all() {
    pool().invalidate_all();
}

// ── SessionPool ───────────────────────────────────────────────────────────

/// Cache of `OnvifSession` instances keyed by `(addr, creds_hash)`.
///
/// The pool itself is `Send + Sync` and uses a plain `std::sync::Mutex`
/// — the critical sections (HashMap probe + insert) are short and
/// never span an `await`, so async-aware locking would be overkill.
/// The `OnvifSession::build()` future runs *outside* the lock so a
/// slow camera doesn't stall other devices' lookups.
#[derive(Default)]
pub struct SessionPool {
    sessions: Mutex<HashMap<SessionKey, Arc<OnvifSession>>>,
}

/// Cache key. We hash credentials rather than storing them so we
/// don't keep extra plaintext copies in memory. Hash collisions
/// across different (user, pass) pairs are astronomically unlikely
/// for `DefaultHasher`'s 64-bit output and would cause "wrong
/// session reused" — recoverable via `invalidate(addr)`.
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct SessionKey {
    addr: String,
    creds_hash: u64,
}

impl SessionKey {
    fn new(addr: &str, username: Option<&str>, password: Option<&str>) -> Self {
        let mut h = DefaultHasher::new();
        username.unwrap_or("").hash(&mut h);
        password.unwrap_or("").hash(&mut h);
        Self {
            addr: addr.to_string(),
            creds_hash: h.finish(),
        }
    }
}

impl SessionPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get(
        &self,
        addr: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<Arc<OnvifSession>, OnvifError> {
        let key = SessionKey::new(addr, username, password);

        // Fast path: cache hit.
        if let Some(s) = self.sessions.lock().unwrap().get(&key).cloned() {
            return Ok(s);
        }

        // Miss: build outside the lock (one GetCapabilities round-trip).
        let mut builder = OnvifSession::builder(addr);
        if let (Some(u), Some(p)) = (username, password) {
            builder = builder.with_credentials(u, p);
        }
        let session = Arc::new(builder.build().await?);

        // Two concurrent misses for the same key both build; first
        // inserter wins, the loser's Arc gets dropped. Wasteful in
        // the rare race but never returns mismatched sessions.
        let mut map = self.sessions.lock().unwrap();
        Ok(map.entry(key).or_insert(session).clone())
    }

    pub fn invalidate(&self, addr: &str) {
        self.sessions.lock().unwrap().retain(|k, _| k.addr != addr);
    }

    pub fn invalidate_all(&self) {
        self.sessions.lock().unwrap().clear();
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
impl SessionPool {
    /// Pre-populate the cache without doing any I/O. Tests can then
    /// exercise `get`/`invalidate` against a known state.
    fn insert_for_test(&self, addr: &str, u: Option<&str>, p: Option<&str>, s: Arc<OnvifSession>) {
        let key = SessionKey::new(addr, u, p);
        self.sessions.lock().unwrap().insert(key, s);
    }

    fn cached_count(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use oxvif::transport::{Transport, TransportError};

    // ── SessionKey ────────────────────────────────────────────────────────

    #[test]
    fn key_differs_by_addr() {
        assert_ne!(
            SessionKey::new("http://a", None, None),
            SessionKey::new("http://b", None, None)
        );
    }

    #[test]
    fn key_differs_by_username() {
        assert_ne!(
            SessionKey::new("http://a", Some("alice"), Some("pw")),
            SessionKey::new("http://a", Some("bob"), Some("pw"))
        );
    }

    #[test]
    fn key_differs_by_password() {
        assert_ne!(
            SessionKey::new("http://a", Some("alice"), Some("pw1")),
            SessionKey::new("http://a", Some("alice"), Some("pw2"))
        );
    }

    #[test]
    fn key_treats_anonymous_as_distinct_from_named() {
        assert_ne!(
            SessionKey::new("http://a", None, None),
            SessionKey::new("http://a", Some("alice"), Some("")),
        );
    }

    #[test]
    fn key_stable_across_calls() {
        assert_eq!(
            SessionKey::new("http://a", Some("alice"), Some("pw")),
            SessionKey::new("http://a", Some("alice"), Some("pw"))
        );
    }

    // ── Pool behaviour (with mock-built sessions) ─────────────────────────

    /// Mock transport that returns a single canned `GetCapabilities`
    /// response — enough for `OnvifSession::builder().build()` to
    /// succeed without hitting the network.
    struct CapsTransport;

    #[async_trait]
    impl Transport for CapsTransport {
        async fn soap_post(
            &self,
            _url: &str,
            _action: &str,
            _body: String,
        ) -> Result<String, TransportError> {
            Ok(
                r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                              xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                              xmlns:tt="http://www.onvif.org/ver10/schema">
              <s:Body>
                <tds:GetCapabilitiesResponse>
                  <tds:Capabilities>
                    <tt:Device><tt:XAddr>http://test/onvif/device</tt:XAddr></tt:Device>
                    <tt:Media><tt:XAddr>http://test/onvif/media</tt:XAddr></tt:Media>
                  </tds:Capabilities>
                </tds:GetCapabilitiesResponse>
              </s:Body>
            </s:Envelope>"#
                    .to_string(),
            )
        }
    }

    async fn build_test_session() -> Arc<OnvifSession> {
        Arc::new(
            OnvifSession::builder("http://test/onvif/device")
                .with_transport(Arc::new(CapsTransport))
                .build()
                .await
                .expect("build session"),
        )
    }

    #[tokio::test]
    async fn cache_hit_returns_same_arc() {
        let pool = SessionPool::new();
        let inserted = build_test_session().await;
        pool.insert_for_test("http://test/onvif/device", None, None, inserted.clone());

        let cached = pool
            .get("http://test/onvif/device", None, None)
            .await
            .expect("cached get");
        assert!(
            Arc::ptr_eq(&inserted, &cached),
            "second get should return the cached Arc, not a freshly built one"
        );
    }

    #[tokio::test]
    async fn invalidate_drops_only_target_addr() {
        let pool = SessionPool::new();
        let s_a = build_test_session().await;
        let s_b = build_test_session().await;
        pool.insert_for_test("http://a/onvif/device", None, None, s_a);
        pool.insert_for_test("http://b/onvif/device", None, None, s_b);
        assert_eq!(pool.cached_count(), 2);

        pool.invalidate("http://a/onvif/device");
        assert_eq!(pool.cached_count(), 1);

        // The remaining entry must still be retrievable.
        pool.get("http://b/onvif/device", None, None)
            .await
            .expect("b still cached");
    }

    #[tokio::test]
    async fn invalidate_drops_all_creds_for_addr() {
        let pool = SessionPool::new();
        let s_anon = build_test_session().await;
        let s_alice = build_test_session().await;
        let s_bob = build_test_session().await;
        pool.insert_for_test("http://a/onvif/device", None, None, s_anon);
        pool.insert_for_test("http://a/onvif/device", Some("alice"), Some("pw"), s_alice);
        pool.insert_for_test("http://a/onvif/device", Some("bob"), Some("pw"), s_bob);
        assert_eq!(pool.cached_count(), 3);

        pool.invalidate("http://a/onvif/device");
        assert_eq!(pool.cached_count(), 0);
    }

    #[tokio::test]
    async fn invalidate_all_clears_everything() {
        let pool = SessionPool::new();
        pool.insert_for_test(
            "http://a/onvif/device",
            None,
            None,
            build_test_session().await,
        );
        pool.insert_for_test(
            "http://b/onvif/device",
            None,
            None,
            build_test_session().await,
        );
        assert_eq!(pool.cached_count(), 2);

        pool.invalidate_all();
        assert_eq!(pool.cached_count(), 0);
    }
}
