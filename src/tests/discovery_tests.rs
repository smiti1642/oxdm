use super::discovery_addr;
use crate::state::DeviceEntry;
use oxvif::DiscoveredDevice;

const FIRST: &str = "http://10.0.0.1/onvif/device_service";
const SAVED: &str = "http://192.168.1.2/onvif/device_service";

fn observation(endpoint: &str, xaddrs: &[&str]) -> DiscoveredDevice {
    DiscoveredDevice {
        endpoint: endpoint.into(),
        types: vec![],
        scopes: vec![],
        xaddrs: xaddrs.iter().map(|s| (*s).into()).collect(),
    }
}

fn saved_device(endpoint: &str, manual: bool) -> DeviceEntry {
    DeviceEntry {
        name: "Camera".into(),
        addr: SAVED.into(),
        display_addr: "192.168.1.2".into(),
        firmware: String::new(),
        location: String::new(),
        online: false,
        auth_status: Default::default(),
        manual,
        credentials: None,
        endpoint: endpoint.into(),
        clone_of: None,
    }
}

#[test]
fn rediscovery_keeps_an_advertised_address_despite_reordering() {
    let devices = [saved_device("urn:uuid:cam", false)];
    for addresses in [[FIRST, SAVED], [SAVED, FIRST]] {
        assert_eq!(
            discovery_addr(&observation("urn:uuid:cam", &addresses), &devices),
            SAVED
        );
    }
}

#[test]
fn withdrawn_address_falls_back_to_the_new_advertisement() {
    assert_eq!(
        discovery_addr(
            &observation("urn:uuid:cam", &[FIRST]),
            &[saved_device("urn:uuid:cam", false)]
        ),
        FIRST
    );
}

#[test]
fn new_device_uses_first_address_or_empty_when_none_advertised() {
    assert_eq!(
        discovery_addr(&observation("new", &[FIRST, SAVED]), &[]),
        FIRST
    );
    assert_eq!(discovery_addr(&observation("new", &[]), &[]), "");
}

#[test]
fn unrelated_manual_and_endpointless_entries_do_not_supply_an_address() {
    for (endpoint, saved_endpoint, manual) in
        [("new", "old", false), ("cam", "cam", true), ("", "", false)]
    {
        assert_eq!(
            discovery_addr(
                &observation(endpoint, &[FIRST, SAVED]),
                &[saved_device(saved_endpoint, manual)]
            ),
            FIRST
        );
    }
}
