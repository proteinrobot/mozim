// SPDX-License-Identifier: Apache-2.0

use crate::{
    integ_tests::env::get_test_veth_cli_mac, DhcpV4Client, DhcpV4Config,
    DhcpV4Lease,
};

use super::env::{DhcpServerEnv, FOO1_STATIC_IP, TEST_NIC_CLI};

const POLL_WAIT_TIME: u32 = 5;

#[test]
fn test_dhcpv4_get_ip() {
    let _srv = DhcpServerEnv::start();

    let mut config = DhcpV4Config::new(TEST_NIC_CLI);
    config.set_host_name("foo1");

    let mut cli = DhcpV4Client::init(config, None).unwrap();

    let lease = get_lease(&mut cli);
    assert!(lease.is_some());
    if let Some(lease) = lease {
        assert_eq!(lease.yiaddr, FOO1_STATIC_IP,);
    }
}

#[test]
fn test_dhcpv4_host_name() {
    let _srv = DhcpServerEnv::start();

    let mut config = DhcpV4Config::new(TEST_NIC_CLI);
    config.set_host_name("foo1");

    let mut cli = DhcpV4Client::init(config, None).unwrap();

    let lease = get_lease(&mut cli);
    assert!(lease.is_some());
    if let Some(lease) = lease {
        assert_eq!(lease.host_name.as_ref(), Some(&"foo1".to_string()));
    }
}

#[test]
fn test_dhcpv4_use_host_name_as_client_id() {
    let _srv = DhcpServerEnv::start();
    let mut config = DhcpV4Config::new(TEST_NIC_CLI);
    config.set_host_name("foo1");
    config.use_host_name_as_client_id();

    let mut cli = DhcpV4Client::init(config.clone(), None).unwrap();

    let lease = get_lease(&mut cli);
    let srv_lease = DhcpServerEnv::get_latest_lease();

    assert!(lease.is_some());
    assert!(srv_lease.is_some());

    if let Some(srv_lease) = srv_lease {
        assert_eq!(srv_lease.client_id, config.client_id,);
    }
}

#[test]
fn test_dhcpv4_use_mac_as_client_id() {
    let _srv = DhcpServerEnv::start();
    let mut config = DhcpV4Config::new(TEST_NIC_CLI);
    config.use_mac_as_client_id();
    let mut cli: DhcpV4Client = DhcpV4Client::init(config, None).unwrap();

    let lease = get_lease(&mut cli);
    let srv_lease = DhcpServerEnv::get_latest_lease();
    let cli_mac = get_test_veth_cli_mac();

    assert!(lease.is_some());
    assert!(srv_lease.is_some());
    assert!(!cli_mac.is_empty());

    if let Some(srv_lease) = srv_lease {
        assert_eq!(srv_lease.mac, cli_mac);
    }
}

fn get_lease(cli: &mut DhcpV4Client) -> Option<DhcpV4Lease> {
    while let Ok(events) = cli.poll(POLL_WAIT_TIME) {
        for event in events {
            match cli.process(event) {
                Ok(Some(lease)) => {
                    return Some(lease);
                }
                Ok(None) => (),
                Err(_) => {
                    return None;
                }
            }
        }
    }
    None
}
