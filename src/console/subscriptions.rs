use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};

use oxvif::events::{ManagedEventOptions, ManagedEventSubscription};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{api, state::Credentials};

use super::grammar::normalize_target;

#[derive(Clone, Default)]
pub(crate) struct EventSubscriptions(Arc<Mutex<HashMap<String, Weak<Semaphore>>>>);

impl EventSubscriptions {
    pub(crate) async fn acquire(&self, address: &str) -> OwnedSemaphorePermit {
        let key = normalize_target(address).unwrap_or_else(|| address.to_owned());
        let semaphore = {
            let mut targets = self.0.lock().unwrap();
            targets.retain(|_, semaphore| semaphore.strong_count() > 0);
            match targets.get(&key).and_then(Weak::upgrade) {
                Some(semaphore) => semaphore,
                None => {
                    let semaphore = Arc::new(Semaphore::new(1));
                    targets.insert(key, Arc::downgrade(&semaphore));
                    semaphore
                }
            }
        };
        semaphore
            .acquire_owned()
            .await
            .expect("semaphore stays open")
    }
}

pub(crate) struct ViewSubscription {
    subscription: Option<ManagedEventSubscription>,
    permit: Option<OwnedSemaphorePermit>,
}

impl ViewSubscription {
    pub(crate) async fn create(
        address: String,
        credentials: Credentials,
        filter: Option<String>,
        permit: OwnedSemaphorePermit,
    ) -> Result<Self, api::ApiError> {
        // A view can unmount after the camera grants a lease but before the
        // reply arrives. A detached Create still receives and cleans up the lease.
        tokio::spawn(async move {
            let session = api::session_for(&address, &credentials).await?;
            let options = ManagedEventOptions {
                pull_wait: Duration::from_secs(5),
                request_timeout: Duration::from_secs(8),
                page_messages: 20,
                max_recoveries: 10,
                recovery_timeout: Duration::from_secs(45),
                ..ManagedEventOptions::default()
            };
            let subscription = session
                .managed_events_recovering(options, filter.as_deref())
                .await
                .map_err(|error| error.to_string())?;
            Ok(Self {
                subscription: Some(subscription),
                permit: Some(permit),
            })
        })
        .await
        .map_err(|error| error.to_string())?
    }

    pub(crate) fn subscription(&mut self) -> &mut ManagedEventSubscription {
        self.subscription.as_mut().expect("owned subscription")
    }
}

impl Drop for ViewSubscription {
    fn drop(&mut self) {
        let mut subscription = self.subscription.take().expect("owned subscription");
        let permit = self.permit.take();
        subscription.cancellation().cancel();
        tokio::spawn(async move {
            let _permit = permit;
            let report = subscription.close().await;
            if let Some(error) = &report.cleanup_error {
                tracing::warn!(%error, "Events view could not release the pull point");
            }
        });
    }
}
