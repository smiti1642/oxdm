use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

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
    address: String,
    credentials: Credentials,
    pub(crate) url: String,
    permit: Option<OwnedSemaphorePermit>,
}

impl ViewSubscription {
    pub(crate) async fn create(
        address: String,
        credentials: Credentials,
        filter: Option<String>,
        lifetime: &'static str,
        permit: OwnedSemaphorePermit,
    ) -> Result<Self, api::ApiError> {
        // A view can unmount after the camera grants a lease but before the
        // reply arrives. A detached Create still receives and cleans up the lease.
        tokio::spawn(async move {
            let subscription = api::create_pull_subscription(
                &address,
                &credentials,
                filter.as_deref(),
                Some(lifetime),
            )
            .await?;
            Ok(Self {
                address,
                credentials,
                url: subscription.reference_url,
                permit: Some(permit),
            })
        })
        .await
        .map_err(|error| error.to_string())?
    }
}

impl Drop for ViewSubscription {
    fn drop(&mut self) {
        let address = std::mem::take(&mut self.address);
        let credentials = std::mem::take(&mut self.credentials);
        let url = std::mem::take(&mut self.url);
        let permit = self.permit.take();
        tokio::spawn(async move {
            let _permit = permit;
            if !url.is_empty() {
                let _ = api::unsubscribe_events(&address, &credentials, &url).await;
            }
        });
    }
}
