// SPDX-License-Identifier: Apache-2.0

//! `capture check`: reads a packet capture (pcap) of one network link and
//! decides whether a device sent anything while it was sealed
//! (`docs/adr/0014-network-seal-test-target.md`).
//!
//! The capture is taken outside the device, by the emulator, from before the
//! device starts. Frames whose Ethernet source address is the device's are
//! *sent*; every other frame is *received*. The device is sealed from the
//! first frame on, or, with `--seal-marker`, from just after the last sent
//! frame that contains the marker.

use crate::args::Args;
use crate::{CommandResult, Outcome};
use nott_format::text::parse_decimal;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;

/// Largest capture read into memory.
const MAX_CAPTURE_BYTES: u64 = 256 * 1024 * 1024;
/// Largest frame accepted, whatever the capture header says.
const MAX_FRAME_BYTES: usize = 256 * 1024;
/// Offending frames listed before the report stops listing.
const MAX_LISTED: usize = 10;
/// pcap link type for Ethernet.
const LINKTYPE_ETHERNET: u32 = 1;

/// What a frame carries, as far as the report needs to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Arp,
    Dhcp,
    Dns,
    Icmp,
    Tcp,
    Udp,
    Icmpv6,
    Ipv6,
    Other,
}

impl Kind {
    const ALL: [Kind; 9] = [
        Kind::Arp,
        Kind::Dhcp,
        Kind::Dns,
        Kind::Icmp,
        Kind::Tcp,
        Kind::Udp,
        Kind::Icmpv6,
        Kind::Ipv6,
        Kind::Other,
    ];

    fn name(self) -> &'static str {
        match self {
            Kind::Arp => "arp",
            Kind::Dhcp => "dhcp",
            Kind::Dns => "dns",
            Kind::Icmp => "icmp",
            Kind::Tcp => "tcp",
            Kind::Udp => "udp",
            Kind::Icmpv6 => "icmpv6",
            Kind::Ipv6 => "ipv6",
            Kind::Other => "other",
        }
    }

    fn from_name(name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// Classifies an Ethernet frame without VLAN tags.
    fn of(frame: &[u8]) -> Kind {
        let byte = |index: usize| frame.get(index).copied();
        let port = |index: usize| Some(u16::from_be_bytes([byte(index)?, byte(index + 1)?]));
        let udp = |offset: usize| {
            let (source, destination) = (port(offset), port(offset + 2));
            let is = |wanted: &[u16]| {
                [source, destination]
                    .into_iter()
                    .flatten()
                    .any(|port| wanted.contains(&port))
            };
            if is(&[67, 68]) {
                Kind::Dhcp
            } else if is(&[53]) {
                Kind::Dns
            } else {
                Kind::Udp
            }
        };
        match port(12) {
            Some(0x0806) => Kind::Arp,
            Some(0x0800) => {
                let Some(first) = byte(14) else {
                    return Kind::Other;
                };
                let transport = 14 + usize::from(first & 0x0f) * 4;
                match byte(23) {
                    Some(1) => Kind::Icmp,
                    Some(6) => Kind::Tcp,
                    Some(17) => udp(transport),
                    _ => Kind::Other,
                }
            }
            Some(0x86dd) => match byte(20) {
                Some(58) => Kind::Icmpv6,
                Some(6) => Kind::Tcp,
                Some(17) => udp(54),
                _ => Kind::Ipv6,
            },
            _ => Kind::Other,
        }
    }
}

/// One captured frame.
struct Frame<'a> {
    /// Seconds since the first frame.
    time: f64,
    data: &'a [u8],
}

/// Parses a classic pcap file: either byte order, microsecond or nanosecond
/// timestamps, Ethernet only. A cut-off last record is an error: a capture
/// that may have lost frames proves nothing.
fn parse(bytes: &[u8]) -> Result<Vec<Frame<'_>>, String> {
    let word = |offset: usize, big: bool| -> Option<u32> {
        let raw: [u8; 4] = bytes.get(offset..offset + 4)?.try_into().ok()?;
        Some(if big {
            u32::from_be_bytes(raw)
        } else {
            u32::from_le_bytes(raw)
        })
    };
    let magic = word(0, false).ok_or("not a pcap file: too short")?;
    let (big, nanoseconds) = match magic {
        0xa1b2_c3d4 => (false, false),
        0xa1b2_3c4d => (false, true),
        0xd4c3_b2a1 => (true, false),
        0x4d3c_b2a1 => (true, true),
        _ => return Err("not a pcap file: unknown magic number".to_owned()),
    };
    let header = |offset| word(offset, big).ok_or("not a pcap file: header cut off");
    let version = header(4)?;
    if version & 0xffff != 2 {
        return Err(format!("unsupported pcap version {}", version & 0xffff));
    }
    if header(20)? != LINKTYPE_ETHERNET {
        return Err("only Ethernet captures are supported".to_owned());
    }
    let scale = if nanoseconds { 1e-9 } else { 1e-6 };
    let mut frames = Vec::new();
    let mut offset = 24;
    let mut start = None;
    while offset < bytes.len() {
        let Some(header) = bytes.get(offset..offset + 16) else {
            return Err("capture cut off in a record header".to_owned());
        };
        let record = |field: usize| {
            let raw = [
                header[field],
                header[field + 1],
                header[field + 2],
                header[field + 3],
            ];
            if big {
                u32::from_be_bytes(raw)
            } else {
                u32::from_le_bytes(raw)
            }
        };
        let seconds = f64::from(record(0)) + f64::from(record(4)) * scale;
        let length = record(8) as usize;
        if length > MAX_FRAME_BYTES {
            return Err(format!(
                "frame {} is larger than {MAX_FRAME_BYTES} bytes",
                frames.len() + 1
            ));
        }
        let data = bytes
            .get(offset + 16..offset + 16 + length)
            .ok_or("capture cut off in a frame")?;
        let first = *start.get_or_insert(seconds);
        frames.push(Frame {
            time: seconds - first,
            data,
        });
        offset += 16 + length;
    }
    Ok(frames)
}

fn parse_mac(text: &str) -> Option<[u8; 6]> {
    let mut mac = [0u8; 6];
    let mut parts = text.split(':');
    for byte in &mut mac {
        let part = parts.next()?;
        if part.len() != 2 {
            return None;
        }
        *byte = u8::from_str_radix(part, 16).ok()?;
    }
    parts.next().is_none().then_some(mac)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn format_mac(mac: &[u8]) -> String {
    mac.iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

pub(crate) fn check(raw: &[&str]) -> CommandResult {
    let args = Args::parse(
        raw,
        &["device-mac", "seal-marker", "expect-sent", "min-received"],
    )?;
    let [path] = args.positionals()?;
    let device = parse_mac(args.required("device-mac")?)
        .ok_or("--device-mac must look like 52:54:00:12:34:56")?;
    let marker = args.optional("seal-marker")?;
    let expected = args
        .repeated("expect-sent")
        .iter()
        .map(|name| {
            Kind::from_name(name).ok_or_else(|| {
                let known: Vec<&str> = Kind::ALL.iter().map(|kind| kind.name()).collect();
                format!("unknown frame kind `{name}`; known: {}", known.join(", "))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !expected.is_empty() && marker.is_none() {
        return Err(
            "--expect-sent needs --seal-marker: nothing may be sent when sealed from the start"
                .to_owned(),
        );
    }
    let min_received = match args.optional("min-received")? {
        None => 0,
        Some(text) => parse_decimal(text)
            .ok_or("--min-received must be a whole number without leading zeros")?,
    };

    let length = fs::metadata(path)
        .map_err(|error| format!("{path}: {error}"))?
        .len();
    if length > MAX_CAPTURE_BYTES {
        return Err(format!("{path}: larger than {MAX_CAPTURE_BYTES} bytes"));
    }
    let bytes = fs::read(path).map_err(|error| format!("{path}: {error}"))?;
    let frames = parse(&bytes).map_err(|error| format!("{path}: {error}"))?;
    let sent = |frame: &Frame<'_>| frame.data.get(6..12) == Some(&device[..]);

    let mut report = String::new();
    let mut failures = Vec::new();
    let sent_count = frames.iter().filter(|frame| sent(frame)).count();
    let received = frames.len() - sent_count;
    let _ = writeln!(
        report,
        "capture: {} frame(s), {sent_count} sent by {}, {received} received",
        frames.len(),
        format_mac(&device)
    );

    // Frames up to and including `open` were sent while unsealed.
    let open = match marker {
        None => {
            let _ = writeln!(report, "sealed from the first frame");
            None
        }
        Some(text) => {
            let last = frames
                .iter()
                .rposition(|frame| sent(frame) && contains(frame.data, text.as_bytes()));
            match last {
                Some(index) => {
                    let _ = writeln!(
                        report,
                        "sealed after frame {}, the last sent frame containing \"{text}\" (+{:.3} s)",
                        index + 1,
                        frames[index].time
                    );
                    Some(index)
                }
                None => {
                    failures.push(format!("no sent frame contains the seal marker \"{text}\""));
                    // Without the marker the whole capture counts as sealed.
                    None
                }
            }
        }
    };
    let split = open.map_or(0, |index| index + 1);

    let mut before: BTreeMap<Kind, usize> = BTreeMap::new();
    for frame in frames[..split].iter().filter(|frame| sent(frame)) {
        *before.entry(Kind::of(frame.data)).or_default() += 1;
    }
    if marker.is_some() {
        let listed: Vec<String> = before
            .iter()
            .map(|(kind, count)| format!("{} {count}", kind.name()))
            .collect();
        let _ = writeln!(report, "sent before sealing: {}", listed.join(", "));
    }
    for kind in &expected {
        if !before.contains_key(kind) {
            failures.push(format!(
                "control: no {} frame was sent before sealing, so the capture cannot be shown to see one",
                kind.name()
            ));
        }
    }

    let leaked: Vec<(usize, &Frame<'_>)> = frames
        .iter()
        .enumerate()
        .skip(split)
        .filter(|(_, frame)| sent(frame))
        .collect();
    let _ = writeln!(report, "sent while sealed: {}", leaked.len());
    for (index, frame) in leaked.iter().take(MAX_LISTED) {
        failures.push(format!(
            "frame {} sent while sealed (+{:.3} s): {} to {}",
            index + 1,
            frame.time,
            Kind::of(frame.data).name(),
            format_mac(frame.data.get(..6).unwrap_or_default())
        ));
    }
    if leaked.len() > MAX_LISTED {
        failures.push(format!("... and {} more", leaked.len() - MAX_LISTED));
    }
    if (received as u64) < min_received {
        failures.push(format!(
            "only {received} received frame(s), at least {min_received} expected"
        ));
    }

    if failures.is_empty() {
        report.push_str("PASS\n");
        Ok(Outcome::Pass(report))
    } else {
        for failure in failures {
            let _ = writeln!(report, "FAIL: {failure}");
        }
        Ok(Outcome::Fail(report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEVICE: [u8; 6] = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
    const HOST: [u8; 6] = [0x52, 0x55, 0x0a, 0x00, 0x02, 0x02];

    fn ethernet(source: [u8; 6], ethertype: u16, payload: &[u8]) -> Vec<u8> {
        let mut frame = vec![0xff; 6];
        frame.extend_from_slice(&source);
        frame.extend_from_slice(&ethertype.to_be_bytes());
        frame.extend_from_slice(payload);
        frame
    }

    fn ipv4(protocol: u8, transport: &[u8]) -> Vec<u8> {
        let mut packet = vec![0x45, 0, 0, 0, 0, 0, 0, 0, 64, protocol, 0, 0];
        packet.extend_from_slice(&[10, 0, 2, 15, 10, 0, 2, 2]);
        packet.extend_from_slice(transport);
        packet
    }

    fn udp(source: u16, destination: u16, data: &[u8]) -> Vec<u8> {
        let mut datagram = source.to_be_bytes().to_vec();
        datagram.extend_from_slice(&destination.to_be_bytes());
        datagram.extend_from_slice(&[0, 0, 0, 0]);
        datagram.extend_from_slice(data);
        datagram
    }

    fn pcap(frames: &[Vec<u8>], big: bool) -> Vec<u8> {
        let word = |value: u32| {
            if big {
                value.to_be_bytes()
            } else {
                value.to_le_bytes()
            }
        };
        let mut file = Vec::new();
        file.extend_from_slice(&word(0xa1b2_c3d4));
        file.extend_from_slice(&word(0x0004_0002));
        file.extend_from_slice(&[0; 8]);
        file.extend_from_slice(&word(65_535));
        file.extend_from_slice(&word(LINKTYPE_ETHERNET));
        for (index, frame) in frames.iter().enumerate() {
            file.extend_from_slice(&word(1_000 + index as u32));
            file.extend_from_slice(&word(0));
            file.extend_from_slice(&word(frame.len() as u32));
            file.extend_from_slice(&word(frame.len() as u32));
            file.extend_from_slice(frame);
        }
        file
    }

    fn run_on(file: &[u8], options: &[&str]) -> CommandResult {
        // Tests run in parallel threads of one process: one file each.
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let number = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("nott-capture-{}-{number}.pcap", std::process::id()));
        fs::write(&path, file).unwrap();
        let path_text = path.to_str().unwrap().to_owned();
        let mut words = vec![path_text.as_str(), "--device-mac", "52:54:00:12:34:56"];
        words.extend_from_slice(options);
        let result = check(&words);
        let _ = fs::remove_file(&path);
        result
    }

    fn passes(result: &CommandResult) -> bool {
        matches!(result, Ok(Outcome::Pass(_)))
    }

    fn report(result: &CommandResult) -> &str {
        match result {
            Ok(Outcome::Pass(text) | Outcome::Fail(text)) => text,
            Err(text) => text,
        }
    }

    fn dhcp(source: [u8; 6]) -> Vec<u8> {
        ethernet(source, 0x0800, &ipv4(17, &udp(68, 67, b"hostname")))
    }

    fn dns(name: &[u8]) -> Vec<u8> {
        ethernet(DEVICE, 0x0800, &ipv4(17, &udp(40_000, 53, name)))
    }

    #[test]
    fn classifies_frames() {
        assert_eq!(Kind::of(&ethernet(DEVICE, 0x0806, &[0; 28])), Kind::Arp);
        assert_eq!(Kind::of(&dhcp(DEVICE)), Kind::Dhcp);
        assert_eq!(Kind::of(&dns(b"x")), Kind::Dns);
        assert_eq!(
            Kind::of(&ethernet(DEVICE, 0x0800, &ipv4(1, &[8, 0]))),
            Kind::Icmp
        );
        assert_eq!(
            Kind::of(&ethernet(DEVICE, 0x0800, &ipv4(6, &[0; 20]))),
            Kind::Tcp
        );
        assert_eq!(
            Kind::of(&ethernet(DEVICE, 0x0800, &ipv4(17, &udp(1, 9, b"")))),
            Kind::Udp
        );
        let mut ipv6 = vec![0x60, 0, 0, 0, 0, 0, 58, 255];
        ipv6.extend_from_slice(&[0; 32]);
        assert_eq!(Kind::of(&ethernet(DEVICE, 0x86dd, &ipv6)), Kind::Icmpv6);
        ipv6[6] = 0;
        assert_eq!(Kind::of(&ethernet(DEVICE, 0x86dd, &ipv6)), Kind::Ipv6);
        assert_eq!(Kind::of(&ethernet(DEVICE, 0x88cc, &[0; 10])), Kind::Other);
        assert_eq!(Kind::of(&[0; 5]), Kind::Other);
        assert_eq!(Kind::of(&ethernet(DEVICE, 0x0800, &[])), Kind::Other);
    }

    #[test]
    fn sealed_from_the_start() {
        let quiet = pcap(&[dhcp(HOST), dhcp(HOST)], false);
        let result = run_on(&quiet, &["--min-received", "2"]);
        assert!(passes(&result), "{}", report(&result));
        assert!(report(&result).contains("sent while sealed: 0"));

        let leak = pcap(&[dhcp(HOST), dhcp(DEVICE)], false);
        let result = run_on(&leak, &[]);
        assert!(!passes(&result));
        assert!(report(&result).contains("frame 2 sent while sealed (+1.000 s): dhcp"));

        let result = run_on(&quiet, &["--min-received", "3"]);
        assert!(report(&result).contains("only 2 received frame(s)"));
        let empty = pcap(&[], false);
        assert!(!passes(&run_on(&empty, &["--min-received", "1"])));
        assert!(passes(&run_on(&empty, &[])));
    }

    #[test]
    fn sealed_after_the_last_marker() {
        let arp = ethernet(DEVICE, 0x0806, &[0; 28]);
        let frames = [
            arp.clone(),
            dhcp(DEVICE),
            dns(b"\x08open-end\x04nott"),
            dhcp(HOST),
            dns(b"\x08open-end\x04nott"),
            dhcp(HOST),
        ];
        let file = pcap(&frames, false);
        let options = [
            "--seal-marker",
            "open-end",
            "--expect-sent",
            "arp",
            "--expect-sent",
            "dhcp",
            "--min-received",
            "1",
        ];
        let result = run_on(&file, &options);
        assert!(passes(&result), "{}", report(&result));
        assert!(report(&result).contains("sealed after frame 5"));
        assert!(report(&result).contains("sent before sealing: arp 1, dhcp 1, dns 2"));

        let mut leaking = frames.to_vec();
        leaking.push(arp);
        let result = run_on(&pcap(&leaking, false), &options);
        assert!(report(&result).contains("frame 7 sent while sealed (+6.000 s): arp"));

        let result = run_on(
            &file,
            &["--seal-marker", "open-end", "--expect-sent", "tcp"],
        );
        assert!(report(&result).contains("control: no tcp frame"));
        let result = run_on(&file, &["--seal-marker", "absent"]);
        assert!(report(&result).contains("no sent frame contains the seal marker"));
        // A marker in a received frame does not count.
        let received_marker = pcap(&[dns(b"x"), ethernet(HOST, 0x0800, b"open-end")], false);
        let result = run_on(&received_marker, &["--seal-marker", "open-end"]);
        assert!(!passes(&result));
    }

    #[test]
    fn reads_both_byte_orders_and_refuses_damaged_captures() {
        let frames = [dhcp(HOST), dhcp(DEVICE)];
        for big in [false, true] {
            let result = run_on(&pcap(&frames, big), &[]);
            assert!(
                report(&result).contains("2 frame(s), 1 sent"),
                "{}",
                report(&result)
            );
        }
        let file = pcap(&frames, false);
        let cut = run_on(&file[..file.len() - 1], &[]);
        assert!(report(&cut).contains("cut off in a frame"));
        let cut = run_on(&file[..file.len() - frames[1].len() - 3], &[]);
        assert!(report(&cut).contains("cut off in a record header"));
        let mut other_link = file.clone();
        other_link[20] = 113;
        assert!(report(&run_on(&other_link, &[])).contains("only Ethernet"));
        assert!(report(&run_on(b"not a capture at all", &[])).contains("unknown magic"));
        assert!(report(&run_on(b"ab", &[])).contains("too short"));
    }

    #[test]
    fn refuses_bad_options() {
        let file = pcap(&[], false);
        assert!(run_on(&file, &["--expect-sent", "arp"]).is_err());
        assert!(run_on(&file, &["--seal-marker", "m", "--expect-sent", "smoke"]).is_err());
        assert!(run_on(&file, &["--min-received", "01"]).is_err());
        assert_eq!(parse_mac("52:54:00:12:34:56"), Some(DEVICE));
        assert_eq!(parse_mac("52:54:00:12:34"), None);
        assert_eq!(parse_mac("52:54:00:12:34:56:78"), None);
        assert_eq!(parse_mac("52:54:00:12:34:5"), None);
        assert_eq!(parse_mac("52:54:00:12:34:zz"), None);
    }
}
