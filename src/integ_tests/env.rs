// SPDX-License-Identifier: Apache-2.0

use std::{
    net::Ipv4Addr,
    process::{Child, Command},
    str::FromStr,
};

const TEST_DHCPD_NETNS: &str = "mozim_test";
pub(crate) const TEST_NIC_CLI: &str = "dhcpcli";
pub(crate) const TEST_PROXY_MAC1: &str = "00:11:22:33:44:55";
const TEST_NIC_SRV: &str = "dhcpsrv";

const TEST_DHCP_SRV_IP: &str = "192.0.2.1";
const TEST_DHCP_LEASEFILE: &str = "/tmp/mozim_test_dhcpd_lease";

pub(crate) const FOO1_STATIC_IP: std::net::Ipv4Addr =
    std::net::Ipv4Addr::new(192, 0, 2, 99);
pub(crate) const TEST_PROXY_IP1: std::net::Ipv4Addr =
    std::net::Ipv4Addr::new(192, 0, 2, 51);

const DNSMASQ_OPTS: &str = r#"
--log-dhcp
--keep-in-foreground
--no-daemon
--conf-file=/dev/null
--no-hosts
--dhcp-host=foo1,192.0.2.99
--dhcp-host=00:11:22:33:44:55,192.0.2.51
--dhcp-option=option:dns-server,8.8.8.8,1.1.1.1
--dhcp-option=option:mtu,1492
--dhcp-option=option:domain-name,example.com
--dhcp-option=option:ntp-server,192.0.2.1
--keep-in-foreground
--bind-interfaces
--except-interface=lo
--clear-on-reload
--listen-address=192.0.2.1
--dhcp-range=192.0.2.2,192.0.2.50,5s --no-ping
"#;

#[derive(Debug)]
pub(crate) struct DhcpServerEnv {
    daemon: Child,
}

#[derive(Debug, PartialEq)]
pub struct DhcpServerLease {
    pub expire: u32,
    pub mac: String,
    pub ip: Ipv4Addr,
    pub host_name: String,
    pub client_id: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseLeaseErr;

impl std::str::FromStr for DhcpServerLease {
    type Err = ParseLeaseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split_whitespace().collect();

        if parts.len() != 5 {
            return Err(ParseLeaseErr);
        }

        let expire = parts[0].parse::<u32>().map_err(|_| ParseLeaseErr)?;
        let mac = parts[1].to_string();

        let ip = Ipv4Addr::from_str(parts[2]).map_err(|_| ParseLeaseErr)?;
        let host_name = parts[3].to_string();
        let client_id: Vec<u8> = parts[4]
            .split(':')
            .map(|hex| u8::from_str_radix(hex, 16))
            .collect::<Result<Vec<u8>, _>>()
            .map_err(|_| ParseLeaseErr)?;

        Ok(DhcpServerLease {
            expire,
            mac,
            ip,
            host_name,
            client_id,
        })
    }
}

impl DhcpServerEnv {
    pub(crate) fn start() -> Self {
        create_test_net_namespace();
        create_test_veth_nics();
        let daemon = start_dhcp_server();
        Self { daemon }
    }

    pub(crate) fn get_latest_lease() -> Option<DhcpServerLease> {
        match std::fs::read_to_string(TEST_DHCP_LEASEFILE) {
            Ok(leases) => match leases.lines().last() {
                Some(lease) => DhcpServerLease::from_str(lease).ok(),
                None => None,
            },
            Err(_) => None,
        }
    }
}

impl Drop for DhcpServerEnv {
    fn drop(&mut self) {
        stop_dhcp_server(&mut self.daemon);
        remove_test_veth_nics();
        remove_test_net_namespace();
    }
}

fn create_test_net_namespace() {
    run_cmd(&format!("ip netns add {TEST_DHCPD_NETNS}"));
}

fn remove_test_net_namespace() {
    run_cmd_ignore_failure(&format!("ip netns del {TEST_DHCPD_NETNS}"));
}

fn create_test_veth_nics() {
    run_cmd(&format!(
        "ip link add {TEST_NIC_CLI} type veth peer name {TEST_NIC_SRV}"
    ));
    run_cmd(&format!("ip link set {TEST_NIC_CLI} up"));
    run_cmd(&format!(
        "ip link set {TEST_NIC_SRV} netns {TEST_DHCPD_NETNS}"
    ));
    run_cmd(&format!(
        "ip netns exec {TEST_DHCPD_NETNS} ip link set {TEST_NIC_SRV} up",
    ));
    run_cmd(&format!(
        "ip netns exec {TEST_DHCPD_NETNS} ip addr add {TEST_DHCP_SRV_IP}/24 dev {TEST_NIC_SRV}",
    ));
}

pub fn get_test_veth_cli_mac() -> String {
    run_cmd(&format!(
        "ip addr show {TEST_NIC_CLI} | grep link/ether | awk '{{print $2}}'"
    ))
    .trim_end()
    .to_string()
}

fn remove_test_veth_nics() {
    run_cmd_ignore_failure(&format!("ip link del {TEST_NIC_CLI}"));
}

fn start_dhcp_server() -> Child {
    let cmd = format!(
        "ip netns exec {} dnsmasq {} --dhcp-leasefile={TEST_DHCP_LEASEFILE}",
        TEST_DHCPD_NETNS,
        DNSMASQ_OPTS.replace('\n', " ")
    );
    let cmds: Vec<&str> = cmd.split(' ').collect();
    let mut child = Command::new(cmds[0])
        .args(&cmds[1..])
        .spawn()
        .expect("Failed to start DHCP server");
    std::thread::sleep(std::time::Duration::from_secs(1));
    if let Ok(Some(ret)) = child.try_wait() {
        panic!("Failed to start DHCP server {ret:?}");
    }
    child
}

fn stop_dhcp_server(daemon: &mut Child) {
    daemon.kill().expect("Failed to stop DHCP server");
    run_cmd(&format!("rm -f {TEST_DHCP_LEASEFILE}"));
}

fn run_cmd(cmd: &str) -> String {
    String::from_utf8(
        Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .unwrap_or_else(|_| panic!("failed to execute command {cmd}"))
            .stdout,
    )
    .expect("Failed to convert file command output to String")
}

fn run_cmd_ignore_failure(cmd: &str) -> String {
    match Command::new("sh").arg("-c").arg(cmd).output() {
        Ok(o) => String::from_utf8(o.stdout).unwrap_or_default(),
        Err(e) => {
            eprintln!("Failed to execute command {cmd}: {e}");
            "".to_string()
        }
    }
}
