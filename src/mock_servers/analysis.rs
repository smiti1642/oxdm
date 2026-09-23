//! Associate reports from one frozen store by insertion ordinal, never by key.

use oxvif::metamorph::{
    FixtureStore, OpOutcome, OperationDiff, OperationQuirk, ParseReport, ParseVerdict, QuirkReport,
    SurfaceOp,
};
use std::collections::HashSet;

#[derive(Clone)]
pub(crate) struct AnalyzedFixture {
    pub id: String,
    pub detail: OperationDiff,
    pub quirk: Option<OperationQuirk>,
    pub verdict: ParseVerdict,
}

#[derive(Clone)]
pub(crate) struct AnalysisView {
    pub report: QuirkReport,
    pub rows: Vec<AnalyzedFixture>,
    pub outcomes: Vec<(SurfaceOp, OpOutcome)>,
}

impl AnalysisView {
    /// Include clean fixtures: a filtered QuirkReport alone cannot reveal a
    /// collision between one drifting response and one clean response.
    pub fn ambiguous_keys(&self) -> Vec<(String, String)> {
        let mut seen = HashSet::new();
        let mut ambiguous = std::collections::BTreeSet::new();
        for row in &self.rows {
            let key = (row.detail.action.clone(), row.detail.key_canon.clone());
            if !seen.insert(key.clone()) {
                ambiguous.insert(key);
            }
        }
        ambiguous.into_iter().collect()
    }

    pub fn selected_quirks(&self, selected: &HashSet<String>) -> Vec<&OperationQuirk> {
        self.rows
            .iter()
            .filter(|row| selected.contains(&row.id))
            .filter_map(|row| row.quirk.as_ref())
            .collect()
    }
}

pub(super) fn assemble(
    generation: u64,
    store: &FixtureStore,
    details: Vec<OperationDiff>,
    report: QuirkReport,
    parsed: ParseReport,
    outcomes: Vec<(SurfaceOp, OpOutcome)>,
) -> Result<AnalysisView, &'static str> {
    const MISMATCH: &str = "Analysis reports do not match the recorded snapshot";
    if details.len() != store.len()
        || parsed.verdicts.len() != store.len()
        || report.compared != store.len()
    {
        return Err(MISMATCH);
    }
    // oxvif 0.17 emits structural quirks in insertion order, omitting only
    // details whose `differs` is false. Restore their full-store ordinals by
    // that predicate, not by indexing the filtered report or by key lookup.
    // This invariant is exercised with actual colliding recordings below.
    let mut quirks = report.quirks.iter();
    let mut rows = Vec::with_capacity(store.len());
    for (ordinal, ((detail, verdict), fixture)) in details
        .into_iter()
        .zip(parsed.verdicts)
        .zip(store.fixtures())
        .enumerate()
    {
        if detail.action != fixture.action
            || detail.key_canon != fixture.key_canon
            || verdict.action != fixture.action
            || verdict.key_canon != fixture.key_canon
        {
            return Err(MISMATCH);
        }
        let quirk = if detail.differs {
            let quirk = quirks.next().ok_or(MISMATCH)?;
            if quirk.action != detail.action || quirk.key_canon != detail.key_canon {
                return Err(MISMATCH);
            }
            Some(quirk.clone())
        } else {
            None
        };
        rows.push(AnalyzedFixture {
            id: format!("{generation}:{ordinal}"),
            detail,
            quirk,
            verdict,
        });
    }
    if quirks.next().is_some() {
        return Err(MISMATCH);
    }
    Ok(AnalysisView {
        report,
        rows,
        outcomes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvif::{mock::MockTransport, transport::Transport};

    const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetHostname";

    fn request(value: &str) -> String {
        oxvif::soap::SoapEnvelope::new(format!(
            "<tds:GetHostname xmlns:tds='http://www.onvif.org/ver10/device/wsdl'><tds:Created>{value}</tds:Created></tds:GetHostname>"
        )).build()
    }

    async fn collision_store() -> FixtureStore {
        let transport = MockTransport::new();
        let clean = request("first");
        let response = transport
            .soap_post("http://mock", ACTION, clean.clone())
            .await
            .unwrap();
        let mut store = FixtureStore::new("collision-control");
        store.record(ACTION, &clean, &response);
        store.record(ACTION, &request("second"), "<second-invalid-response/>");
        store.record(ACTION, &request("third"), "<third-invalid-response/>");
        assert_eq!(store.len(), 3);
        assert!(store
            .fixtures()
            .iter()
            .all(|f| f.key_canon == store.fixtures()[0].key_canon));
        store
    }

    async fn analyze(store: FixtureStore, id: u64) -> AnalysisView {
        super::super::Analysis {
            id,
            store,
            sweep: Default::default(),
        }
        .analyze()
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn collision_rows_keep_individual_xml_verdicts_quirks_and_selection() {
        let store = collision_store().await;
        let view = analyze(store.clone(), 10).await;
        assert_eq!(view.rows.len(), 3);
        assert!(
            view.rows[0].quirk.is_none(),
            "clean response must not acquire its sibling's quirk"
        );
        for (row, marker) in view.rows[1..]
            .iter()
            .zip(["second-invalid-response", "third-invalid-response"])
        {
            assert_eq!(row.verdict.status, oxvif::metamorph::ParseStatus::Failed);
            assert!(row.detail.clone_xml.contains(marker));
            let q = row.quirk.as_ref().unwrap();
            assert!(q.only_in_clone.iter().any(|path| path.contains(marker)));
            let selected = HashSet::from([row.id.clone()]);
            assert_eq!(view.selected_quirks(&selected), vec![q]);
        }
        let selection = HashSet::from([view.rows[1].id.clone(), view.rows[2].id.clone()]);
        assert_eq!(view.selected_quirks(&selection).len(), 2);
        let next = analyze(store.clone(), 11).await;
        assert!(
            next.selected_quirks(&selection).is_empty(),
            "new snapshot cannot inherit old selection"
        );

        // Also exercise a clean record after changed records, not just before.
        let mut reversed = FixtureStore::new("reversed");
        for f in store.fixtures().iter().rev() {
            reversed.record(&f.action, &f.request_raw, &f.response_raw);
        }
        let reversed = analyze(reversed, 12).await;
        assert!(reversed.rows[2].quirk.is_none());
        assert_eq!(reversed.rows[0].quirk, view.rows[2].quirk);

        let mut one_quirk = FixtureStore::new("filtered-singleton");
        for f in &store.fixtures()[..2] {
            one_quirk.record(&f.action, &f.request_raw, &f.response_raw);
        }
        let one_quirk = analyze(one_quirk, 13).await;
        assert_eq!(one_quirk.report.quirks.len(), 1);
        assert_eq!(
            one_quirk.ambiguous_keys().len(),
            1,
            "clean sibling still makes the report key ambiguous"
        );
    }

    #[tokio::test]
    async fn different_actions_with_one_key_remain_distinct() {
        let mut store = FixtureStore::new("media");
        for (ns, marker) in [
            ("http://www.onvif.org/ver10/media/wsdl", "media-one"),
            ("http://www.onvif.org/ver20/media/wsdl", "media-two"),
        ] {
            let req =
                oxvif::soap::SoapEnvelope::new(format!("<m:GetProfiles xmlns:m='{ns}'/>")).build();
            store.record(&format!("{ns}/GetProfiles"), &req, &format!("<{marker}/>"));
        }
        assert_eq!(store.fixtures()[0].key_canon, store.fixtures()[1].key_canon);
        let view = analyze(store, 20).await;
        assert_ne!(view.rows[0].id, view.rows[1].id);
        assert_ne!(view.rows[0].verdict.action, view.rows[1].verdict.action);
        assert!(view.rows[0].detail.clone_xml.contains("media-one"));
        assert!(view.rows[1].detail.clone_xml.contains("media-two"));
    }

    #[tokio::test]
    async fn incomplete_or_wrong_reports_are_rejected_instead_of_truncated() {
        let store = collision_store().await;
        let details = store.diff_details();
        let report = store.diff_against_synthetic();
        let parsed = store.verify_parsing().await;
        let mut short = parsed.clone();
        short.verdicts.pop();
        assert!(assemble(1, &store, details.clone(), report.clone(), short, vec![]).is_err());
        let mut wrong = parsed.clone();
        wrong.verdicts[0].action = "wrong".into();
        assert!(assemble(1, &store, details.clone(), report.clone(), wrong, vec![]).is_err());
        let mut short = report.clone();
        short.quirks.pop();
        assert!(assemble(1, &store, details, short, parsed, vec![]).is_err());
    }

    #[tokio::test]
    async fn incomplete_legacy_metadata_is_not_reported_as_parsed() {
        let mut store = FixtureStore::new("legacy-metadata");
        let request = oxvif::soap::SoapEnvelope::new("<tr2:GetMetadataConfigurations/>".into())
            .with_wsa_to("http://mock/media2")
            .build();
        let response = oxvif::soap::SoapEnvelope::new("<tr2:GetMetadataConfigurationsResponse><tr2:Configurations token='meta'><tt:Name>legacy</tt:Name><tt:UseCount>1</tt:UseCount></tr2:Configurations></tr2:GetMetadataConfigurationsResponse>".into()).build();
        store.record(
            "http://www.onvif.org/ver20/media/wsdl/GetMetadataConfigurations",
            &request,
            &response,
        );
        let view = analyze(store.clone(), 40).await;
        // The 0.17 metamorph verifier has no metadata parser dispatch. Preserve
        // Unverified rather than claiming success or inventing missing fields.
        assert_eq!(
            view.rows[0].verdict.status,
            oxvif::metamorph::ParseStatus::Unverified
        );
        let client = oxvif::OnvifClient::new("http://mock/device").with_transport(
            std::sync::Arc::new(oxvif::metamorph::MetamorphTransport::new(store.clone())),
        );
        let error = client
            .get_metadata_configurations_media2("http://mock/media2", None, None)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Multicast"));
        assert_eq!(store.fixtures()[0].response_raw, response);
        assert!(!view.rows[0].detail.clone_xml.contains("SessionTimeout"));
    }

    #[tokio::test]
    async fn legacy_file_load_is_read_only_and_collision_roundtrip_retains_all_rows() {
        let store = collision_store().await;
        let dir = std::env::temp_dir().join(format!(
            "oxdm-017-fixtures-{}-{}",
            std::process::id(),
            crate::util::now_file_stamp()
        ));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("fixtures.json");
        // The legacy format has no schema version and carries raw requests.
        let bytes = serde_json::to_vec(
            &serde_json::json!({"device":store.device(), "fixtures":store.fixtures()}),
        )
        .unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let loaded = FixtureStore::load(&dir).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        loaded.save(&dir).unwrap();
        let reloaded = FixtureStore::load(&dir).unwrap();
        assert_eq!(reloaded.len(), 3);
        for (old, new) in store.fixtures().iter().zip(reloaded.fixtures()) {
            assert_eq!(old.request_raw, new.request_raw);
            assert_eq!(old.response_raw, new.response_raw);
        }
        let server = oxvif::mock::MockServer::builder()
            .replay(reloaded.clone())
            .start()
            .await
            .unwrap();
        let http = oxvif::transport::HttpTransport::new();
        for fixture in reloaded.fixtures() {
            let response = http
                .soap_post(
                    server.device_url(),
                    &fixture.action,
                    fixture.request_raw.clone(),
                )
                .await
                .unwrap();
            assert_eq!(
                response, fixture.response_raw,
                "each saved request selects its own replay response"
            );
        }
        let view = analyze(reloaded, 30).await;
        assert_eq!(view.rows.len(), 3);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
