//! What a configuration uses of what must not clash or overflow, in one
//! place, for `--summary`: every address and port, every device id, every
//! message ID and APID, and how much it sends the ground.
//!
//! Nothing here is a rule. Each list is `tcsprotocol`'s -- the sockets
//! [`tcsprotocol::graph::sockets::of`], the ids [`telemetry::ids`], the
//! message IDs [`telemetry::msgids`], the APIDs [`telemetry::apids`], the rates
//! [`tcsprotocol::graph::rates::of`] -- and the checks that hold them apart
//! are `Graph::check`'s, which the run has printed above. This lays them out:
//! plain text in aligned columns, each list in the order it is made in, so that
//! two summaries can be compared with `diff`.

use std::fmt::Write;
use std::path::Path;

use tcsprotocol::graph::sockets::Socket;
use tcsprotocol::graph::telemetry::{self, Telemetered};
use tcsprotocol::graph::Graph;

/// Lays out rows in columns, two spaces apart, each as wide as its widest
/// cell, the last left ragged.
///
/// # Arguments
///
/// * `out`   - Where it is written.
/// * `right` - For each column, whether it is aligned to the right, as
///             numbers are.
/// * `rows`  - The rows, the heading first, each of as many cells as
///             `right` has.
fn table(out: &mut String, right: &[bool], rows: &[Vec<String>]) {
    let mut widths = vec![0; right.len()];
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    for row in rows {
        let mut line = String::from(" ");
        for (at, cell) in row.iter().enumerate() {
            let last = at + 1 == row.len();
            let width = widths[at];
            line.push(' ');
            if right[at] {
                let _ = write!(line, "{cell:>width$}");
            } else if last {
                line.push_str(cell);
            } else {
                let _ = write!(line, "{cell:<width$}");
            }
            if !last {
                line.push(' ');
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
}

/// Writes a number of data or bytes a second as briefly as it is exact to
/// a thousandth.
///
/// # Arguments
///
/// * `value` - The number.
///
/// # Returns
///
/// It, without trailing noughts: `3`, `0.2`, `0.333`.
fn number(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    text.to_string()
}

/// Writes the addresses and ports.
///
/// # Arguments
///
/// * `out`     - Where it is written.
/// * `sockets` - Every socket, in [`tcsprotocol::graph::sockets::of`]'s
///   order.
fn addresses(out: &mut String, sockets: &[Socket]) {
    out.push_str("Addresses and ports\n");
    if sockets.is_empty() {
        out.push_str("  none\n");
        return;
    }
    let mut rows = vec![vec![
        "address".to_string(),
        "bound by".to_string(),
        "what".to_string(),
    ]];
    for socket in sockets {
        let mut what = socket.what.clone();
        if socket.port == 0 {
            what.push_str(" (any port)");
        }
        rows.push(vec![
            format!("{}:{}", socket.address, socket.port),
            socket.bound_by.word().to_string(),
            what,
        ]);
    }
    table(out, &[false, false, false], &rows);
}

/// Writes every node's device id, which every counted port and ID is
/// counted by.
///
/// # Arguments
///
/// * `out`   - Where it is written.
/// * `graph` - The configuration.
fn device_ids(out: &mut String, graph: &Graph) {
    out.push_str("Device ids\n");
    let mut ids: Vec<(usize, String)> = telemetry::ids(graph)
        .into_iter()
        .map(|(name, id)| (id, name))
        .collect();
    ids.sort();
    let mut rows = vec![vec!["id".to_string(), "node".to_string()]];
    for (id, name) in ids {
        rows.push(vec![id.to_string(), name]);
    }
    table(out, &[true, false], &rows);
}

/// Writes the message IDs, where the configuration is on a software bus.
///
/// # Arguments
///
/// * `out`   - Where it is written.
/// * `graph` - The configuration.
fn msgids(out: &mut String, graph: &Graph) {
    out.push_str("Message IDs\n");
    let mut used = match telemetry::msgids(graph) {
        Ok(used) => used,
        Err(refused) => {
            let _ = writeln!(out, "  not listed: {}", refused.why);
            return;
        }
    };
    if used.is_empty() {
        out.push_str("  none: the configuration is not on a software bus\n");
        return;
    }
    // The allotment every ID is held to, where the section reads, which it
    // does: the IDs were found from it.
    let bus = graph.tcspecial.as_ref().and_then(|section| {
        let flat = tcsprotocol::graph::parameters::flatten(section);
        tcsprotocol::graph::parameters::ci_config_of(&flat)
            .ok()?
            .to_ci_config()
            .ok()?
            .bus()
            .cloned()
    });
    if let Some(bus) = bus {
        let _ = writeln!(
            out,
            "  msgid_range {:#06x} to {:#06x}",
            bus.first, bus.last
        );
    }
    used.sort_by_key(|one| one.msgid);
    let mut rows = vec![vec![
        "ID".to_string(),
        "carries".to_string(),
        "what".to_string(),
    ]];
    for one in used {
        rows.push(vec![
            format!("{:#06x}", one.msgid),
            if one.command { "command" } else { "telemetry" }.to_string(),
            one.what,
        ]);
    }
    table(out, &[false, false, false], &rows);
}

/// Writes the APIDs, where the configuration's packets are CCSDS Space
/// Packets.
///
/// # Arguments
///
/// * `out`   - Where it is written.
/// * `graph` - The configuration.
fn apids(out: &mut String, graph: &Graph) {
    out.push_str("APIDs\n");
    let mut used = match telemetry::apids(graph) {
        Ok(used) => used,
        Err(refused) => {
            let _ = writeln!(out, "  not listed: {}", refused.why);
            return;
        }
    };
    if used.is_empty() {
        out.push_str(
            "  none: the configuration's packets carry no CCSDS header\n",
        );
        return;
    }
    used.sort_by_key(|one| one.apid);
    let mut rows = vec![vec!["APID".to_string(), "what".to_string()]];
    for one in used {
        rows.push(vec![format!("{:#05x}", one.apid), one.what]);
    }
    table(out, &[false, false], &rows);
}

/// Writes how much each link to the ground carries, the beacon's, and the
/// sum.
///
/// # Arguments
///
/// * `out`   - Where it is written.
/// * `graph` - The configuration.
fn rates(out: &mut String, graph: &Graph) {
    out.push_str("Rates to the ground\n");
    let mut links: Vec<Telemetered> = match telemetry::of(graph) {
        Ok(links) => links,
        Err(refused) => {
            let _ = writeln!(out, "  not listed: {}", refused.why);
            return;
        }
    };
    links.sort_by(|a, b| a.name.cmp(&b.name));
    let rates = tcsprotocol::graph::rates::of(graph);
    let mut rows = vec![vec![
        "link".to_string(),
        "data/s".to_string(),
        "bytes/datum".to_string(),
        "bytes/s".to_string(),
        "how".to_string(),
    ]];
    let (mut data, mut bytes) = (0.0, 0.0);
    let mut unknown = 0;
    let mut at_most = false;
    for link in &links {
        let rate = rates.get(&link.source);
        let per_second = rate.and_then(|rate| rate.per_second);
        let bound = rate.is_some_and(|rate| rate.at_most);
        let width = link.carries.width();
        let shown = |value: Option<f64>| match value {
            Some(value) if bound => format!("<={}", number(value)),
            Some(value) => number(value),
            None => "?".to_string(),
        };
        match per_second {
            Some(value) => {
                data += value;
                at_most |= bound;
            }
            None => unknown += 1,
        }
        let per_byte = per_second.zip(width).map(|(r, w)| r * w as f64);
        if let Some(value) = per_byte {
            bytes += value;
        }
        rows.push(vec![
            link.name.clone(),
            shown(per_second),
            width.map_or_else(|| "?".to_string(), |w| w.to_string()),
            shown(per_byte),
            rate.map_or_else(String::new, |rate| rate.how.clone()),
        ]);
    }
    if let Ok(Some(every)) =
        tcsprotocol::graph::parameters::beacon_interval_of(graph)
    {
        if !every.is_zero() {
            let value = 1.0 / every.as_secs_f64();
            data += value;
            rows.push(vec![
                "beacon".to_string(),
                number(value),
                "?".to_string(),
                "?".to_string(),
                format!(
                    "beacon.default_interval {} s",
                    number(every.as_secs_f64())
                ),
            ]);
        }
    }
    let bound = if at_most { "<=" } else { "" };
    rows.push(vec![
        "total".to_string(),
        format!("{bound}{}", number(data)),
        String::new(),
        format!("{bound}{}", number(bytes)),
        match unknown {
            0 => String::new(),
            1 => "not counting one link of unknown rate".to_string(),
            n => format!("not counting {n} links of unknown rate"),
        },
    ]);
    table(out, &[false, true, true, true, false], &rows);
}

/// Lays out what a configuration uses of what must not clash or overflow.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// The summary: addresses and ports, device ids, message IDs, APIDs and
/// rates, each under its heading, a blank line between them.
pub fn written(graph: &Graph) -> String {
    let mut out = String::new();
    addresses(&mut out, &tcsprotocol::graph::sockets::of(graph));
    out.push('\n');
    device_ids(&mut out, graph);
    out.push('\n');
    msgids(&mut out, graph);
    out.push('\n');
    apids(&mut out, graph);
    out.push('\n');
    rates(&mut out, graph);
    out
}

/// Reads a configuration and lays out its summary.
///
/// # Arguments
///
/// * `configuration` - The configuration's file.
///
/// # Returns
///
/// The summary, headed by the file's name.
///
/// # Errors
///
/// A message, naming the file, where it does not read.
pub fn of(configuration: &Path) -> Result<String, String> {
    let shown = configuration.display();
    let graph =
        Graph::load(configuration).map_err(|why| format!("{shown}: {why}"))?;
    Ok(format!("{shown}: summary\n\n{}", written(&graph)))
}
