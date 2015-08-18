use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use super::graphs::{Graph, NodeLinks};
use super::graphs::NodeLinks::{Full, Isolate, DropSome};
use super::super::container::nsutil::set_namespace;
use super::super::container::container::Namespace::NewNet;


fn _rule<W: Write, S:AsRef<str>>(out: &mut W, data: S) -> Result<(), String> {
    debug!("Rule: {}", data.as_ref());
    (writeln!(out, "{}", data.as_ref()))
    .map_err(|e| format!("Error piping firewall rule {:?}: {}",
        data.as_ref(), e))
}

pub fn apply_graph(graph: Graph) -> Result<(), String> {
    for (ip, node) in graph.nodes.iter() {
        try!(apply_node(ip, node));
    }
    Ok(())
}

fn apply_node(ip: &String, node: &NodeLinks) -> Result<(), String> {
    try!(set_namespace(
        &Path::new(&format!("/tmp/vagga/namespaces/net.{}", ip)), NewNet)
        .map_err(|e| format!("Can't set namespace: {}", e)));
    let mut cmd = Command::new("iptables-restore");
    cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    debug!("Running {:?} for {}", cmd, ip);
    let mut prc = try!(cmd.spawn()
        .map_err(|e| format!("Can't run iptables-restore: {}", e)));
    {
        let pipe = prc.stdin.as_mut().unwrap();

        try!(_rule(pipe, "*filter"));
        match *node {
            Full => {
                // Empty chains with ACCEPT default (except FORWARD, we expect
                // user doesn't use FORWARD, i.e. nested networks)
                try!(_rule(pipe, ":INPUT ACCEPT [0:0]"));
                try!(_rule(pipe, ":FORWARD DROP [0:0]"));
                try!(_rule(pipe, ":OUTPUT ACCEPT [0:0]"));
            }
            Isolate => {
                // The DROP default and accept packets to/from bridge as an
                // exception
                try!(_rule(pipe, ":INPUT DROP [0:0]"));
                try!(_rule(pipe, ":FORWARD DROP [0:0]"));
                try!(_rule(pipe, ":OUTPUT DROP [0:0]"));
                try!(_rule(pipe,
                    format!("-A INPUT -s 172.18.0.254/32 -j ACCEPT")));
                try!(_rule(pipe,
                    format!("-A OUTPUT -d 172.18.0.254/32 -j ACCEPT")));
            }
            DropSome(ref peers) => {
                // Empty chains with ACCEPT default (except FORWARD, we expect
                // user doesn't use FORWARD, i.e. nested networks)
                try!(_rule(pipe, ":INPUT ACCEPT [0:0]"));
                try!(_rule(pipe, ":FORWARD DROP [0:0]"));
                try!(_rule(pipe, ":OUTPUT ACCEPT [0:0]"));
                for peer in peers.iter() {
                    try!(_rule(pipe,
                        format!("-A INPUT -s {}/32 -d {}/32 -j DROP",
                        ip, peer)));
                }
            }
        }
        try!(_rule(pipe, "COMMIT"));
    }
    match prc.wait() {
        Ok(status) if status.success() => Ok(()),
        e => Err(format!("Error running iptables-restore: {:?}", e)),
    }
}
