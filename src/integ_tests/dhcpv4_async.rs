// SPDX-License-Identifier: Apache-2.0

use futures::StreamExt;

use crate::{DhcpV4ClientAsync, DhcpV4Config, DhcpV4Lease};

use super::env::{DhcpServerEnv, FOO1_STATIC_IP, TEST_NIC_CLI};

#[test]
fn test_dhcpv4_async() {
    let _srv = DhcpServerEnv::start();

    let mut config = DhcpV4Config::new(TEST_NIC_CLI);
    config.set_host_name("foo1");
    config.use_host_name_as_client_id();

    let mut cli = DhcpV4ClientAsync::init(config.clone(), None).unwrap();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .unwrap();

    let lease = rt.block_on(get_lease(&mut cli));
    let srv_lease = DhcpServerEnv::get_latest_lease();
    assert!(lease.is_some());
    assert!(srv_lease.is_some());

    if let (Some(srv_lease), Some(lease)) = (srv_lease, lease) {
        assert_eq!(lease.host_name.as_ref(), Some(&"foo1".to_string()));
        assert_eq!(lease.yiaddr, FOO1_STATIC_IP,);
        assert_eq!(srv_lease.client_id, config.client_id,);
    }
}

async fn get_lease(cli: &mut DhcpV4ClientAsync) -> Option<DhcpV4Lease> {
    cli.next().await.unwrap().ok()
}
