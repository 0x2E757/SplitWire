//! The few parts of the DNS wire format (RFC 1035) SplitWire needs: read a query, read the
//! records of a response (with name compression), patch IDs and TTLs in place, and build
//! empty replies. Upstream answers are passed on byte for byte apart from those patches.

use std::fmt;
use std::net::Ipv4Addr;

pub const TYPE_A: u16 = 1;
pub const TYPE_CNAME: u16 = 5;
pub const TYPE_SOA: u16 = 6;
pub const TYPE_AAAA: u16 = 28;
pub const TYPE_OPT: u16 = 41;
pub const CLASS_IN: u16 = 1;
pub const RCODE_NOERROR: u8 = 0;
pub const RCODE_SERVFAIL: u8 = 2;

const HEADER_LEN: usize = 12;
const FLAG_QR: u8 = 0x80;
const FLAG_TC: u8 = 0x02;
const FLAG_RD: u8 = 0x01;
const FLAG_RA: u8 = 0x80;
const MIN_UDP: usize = 512;
const MAX_UDP: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WireError(pub &'static str);

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "malformed DNS message: {}", self.0)
    }
}

impl std::error::Error for WireError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Question {
    pub name: String,
    pub qtype: u16,
    pub qclass: u16,
}

/// A resource record located inside a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub name: String,
    pub rtype: u16,
    pub class: u16,
    pub ttl: u32,
    ttl_offset: usize,
    rdata_offset: usize,
    rdata_len: usize,
}

/// A client query, kept with its raw bytes so it can be forwarded unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DnsQuery {
    pub id: u16,
    pub question: Question,
    raw: Vec<u8>,
    question_end: usize,
    udp_limit: usize,
}

impl DnsQuery {
    pub fn parse(raw: &[u8]) -> Result<Self, WireError> {
        let header = Header::read(raw)?;
        if header.flags_hi & FLAG_QR != 0 {
            return Err(WireError("not a query"));
        }
        if header.qdcount != 1 {
            return Err(WireError("expected exactly one question"));
        }
        let (question, question_end) = read_question(raw, HEADER_LEN)?;
        let mut pos = question_end;
        let mut udp_limit = MIN_UDP;
        for _ in 0..header.ancount + header.nscount + header.arcount {
            let (record, next) = read_record(raw, pos)?;
            if record.rtype == TYPE_OPT {
                udp_limit = usize::from(record.class).clamp(MIN_UDP, MAX_UDP);
            }
            pos = next;
        }
        Ok(Self {
            id: header.id,
            question,
            raw: raw.to_vec(),
            question_end,
            udp_limit,
        })
    }

    /// A standard recursive query for `name` (used to resolve the bypass names).
    pub fn build(id: u16, name: &str, qtype: u16) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + name.len() + 6);
        out.extend_from_slice(&id.to_be_bytes());
        out.extend_from_slice(&[FLAG_RD, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
        write_name(&mut out, name);
        out.extend_from_slice(&qtype.to_be_bytes());
        out.extend_from_slice(&CLASS_IN.to_be_bytes());
        out
    }

    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// The biggest UDP reply the client accepts (512 without EDNS).
    pub fn udp_limit(&self) -> usize {
        self.udp_limit
    }

    /// A reply with no records and the given rcode (empty NOERROR, SERVFAIL).
    pub fn reply(&self, rcode: u8) -> Vec<u8> {
        self.header_only_reply(rcode, false)
    }

    /// An empty reply with TC set, so the client retries over TCP.
    pub fn truncated_reply(&self) -> Vec<u8> {
        self.header_only_reply(RCODE_NOERROR, true)
    }

    fn header_only_reply(&self, rcode: u8, truncated: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.question_end);
        out.extend_from_slice(&self.id.to_be_bytes());
        let opcode_and_rd = self.raw[2] & 0x79;
        let tc = if truncated { FLAG_TC } else { 0 };
        out.push(FLAG_QR | opcode_and_rd | tc);
        out.push(FLAG_RA | (rcode & 0x0f));
        out.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&self.raw[HEADER_LEN..self.question_end]);
        out
    }
}

/// A response as received, with its records located so IDs and TTLs can be patched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DnsResponse {
    raw: Vec<u8>,
    records: Vec<Record>,
    question_end: usize,
    answer_count: usize,
    authority_count: usize,
}

impl DnsResponse {
    pub fn parse(raw: Vec<u8>) -> Result<Self, WireError> {
        let header = Header::read(&raw)?;
        if header.flags_hi & FLAG_QR == 0 {
            return Err(WireError("not a response"));
        }
        let mut pos = HEADER_LEN;
        for _ in 0..header.qdcount {
            pos = read_question(&raw, pos)?.1;
        }
        let question_end = pos;
        let total =
            usize::from(header.ancount) + usize::from(header.nscount) + usize::from(header.arcount);
        let mut records = Vec::with_capacity(total);
        for _ in 0..total {
            let (record, next) = read_record(&raw, pos)?;
            records.push(record);
            pos = next;
        }
        Ok(Self {
            raw,
            records,
            question_end,
            answer_count: usize::from(header.ancount),
            authority_count: usize::from(header.nscount),
        })
    }

    pub fn id(&self) -> u16 {
        u16::from_be_bytes([self.raw[0], self.raw[1]])
    }

    pub fn rcode(&self) -> u8 {
        self.raw[3] & 0x0f
    }

    pub fn truncated(&self) -> bool {
        self.raw[2] & FLAG_TC != 0
    }

    pub fn answers(&self) -> &[Record] {
        &self.records[..self.answer_count]
    }

    /// The IPv4 addresses `qname` resolves to, following CNAMEs inside the answer section.
    pub fn ipv4_chain(&self, qname: &str) -> Vec<Ipv4Addr> {
        let mut names = vec![normalize(qname)];
        loop {
            let targets: Vec<String> = self
                .answers()
                .iter()
                .filter(|r| r.rtype == TYPE_CNAME && names.contains(&normalize(&r.name)))
                .filter_map(|r| read_name(&self.raw, r.rdata_offset).ok())
                .map(|(target, _)| normalize(&target))
                .filter(|target| !names.contains(target))
                .collect();
            if targets.is_empty() {
                break;
            }
            names.extend(targets);
        }
        let mut ips = Vec::new();
        for record in self.answers() {
            let wanted = record.rtype == TYPE_A
                && record.class == CLASS_IN
                && record.rdata_len == 4
                && names.contains(&normalize(&record.name));
            if wanted {
                let at = record.rdata_offset;
                let ip = Ipv4Addr::new(
                    self.raw[at],
                    self.raw[at + 1],
                    self.raw[at + 2],
                    self.raw[at + 3],
                );
                if !ips.contains(&ip) {
                    ips.push(ip);
                }
            }
        }
        ips
    }

    pub fn min_answer_ttl(&self) -> Option<u32> {
        self.answers().iter().map(|r| r.ttl).min()
    }

    pub fn set_id(&mut self, id: u16) {
        self.raw[..2].copy_from_slice(&id.to_be_bytes());
    }

    /// Lowers every TTL above `max` to `max` (the OPT pseudo-record has no TTL).
    pub fn cap_ttls(&mut self, max: u32) {
        self.patch_ttls(|ttl| ttl.min(max));
    }

    /// Sets every TTL to `ttl` (a cached answer's remaining lifetime).
    pub fn set_ttls(&mut self, ttl: u32) {
        self.patch_ttls(|_| ttl);
    }

    pub fn bytes(&self) -> &[u8] {
        &self.raw
    }

    /// The response without the A records for `dropped`, rebuilt with uncompressed names (a
    /// held address leaves another profile's answer). The authority section is left
    /// out and only the OPT record of the additional section is kept.
    pub fn without_addresses(&self, dropped: &[Ipv4Addr]) -> Result<DnsResponse, WireError> {
        let mut out = self.raw[..self.question_end].to_vec();
        let mut answers: u16 = 0;
        for record in self.answers() {
            if self
                .a_record(record)
                .is_some_and(|ip| dropped.contains(&ip))
            {
                continue;
            }
            let rdata = if record.rtype == TYPE_CNAME {
                let (target, _) = read_name(&self.raw, record.rdata_offset)?;
                let mut rdata = Vec::new();
                write_name(&mut rdata, &target);
                rdata
            } else {
                self.raw[record.rdata_offset..record.rdata_offset + record.rdata_len].to_vec()
            };
            write_name(&mut out, &record.name);
            Self::write_record(&mut out, record, &rdata);
            answers += 1;
        }
        let mut additional: u16 = 0;
        let rest = &self.records[self.answer_count + self.authority_count..];
        for record in rest.iter().filter(|r| r.rtype == TYPE_OPT) {
            out.push(0);
            let rdata = &self.raw[record.rdata_offset..record.rdata_offset + record.rdata_len];
            Self::write_record(&mut out, record, rdata);
            additional += 1;
        }
        out[6..8].copy_from_slice(&answers.to_be_bytes());
        out[8..10].copy_from_slice(&0u16.to_be_bytes());
        out[10..12].copy_from_slice(&additional.to_be_bytes());
        DnsResponse::parse(out)
    }

    fn a_record(&self, record: &Record) -> Option<Ipv4Addr> {
        (record.rtype == TYPE_A && record.class == CLASS_IN && record.rdata_len == 4).then(|| {
            let at = record.rdata_offset;
            Ipv4Addr::new(
                self.raw[at],
                self.raw[at + 1],
                self.raw[at + 2],
                self.raw[at + 3],
            )
        })
    }

    fn write_record(out: &mut Vec<u8>, record: &Record, rdata: &[u8]) {
        out.extend_from_slice(&record.rtype.to_be_bytes());
        out.extend_from_slice(&record.class.to_be_bytes());
        out.extend_from_slice(&record.ttl.to_be_bytes());
        out.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
        out.extend_from_slice(rdata);
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.raw
    }

    fn patch_ttls(&mut self, new_ttl: impl Fn(u32) -> u32) {
        for record in self.records.iter_mut().filter(|r| r.rtype != TYPE_OPT) {
            record.ttl = new_ttl(record.ttl);
            let at = record.ttl_offset;
            self.raw[at..at + 4].copy_from_slice(&record.ttl.to_be_bytes());
        }
    }
}

/// Encodes `name` as uncompressed labels (the root for "").
pub fn write_name(out: &mut Vec<u8>, name: &str) {
    for label in name
        .trim_end_matches('.')
        .split('.')
        .filter(|l| !l.is_empty())
    {
        let bytes = &label.as_bytes()[..label.len().min(63)];
        out.push(bytes.len() as u8);
        out.extend_from_slice(bytes);
    }
    out.push(0);
}

fn normalize(name: &str) -> String {
    name.trim_end_matches('.').to_ascii_lowercase()
}

struct Header {
    id: u16,
    flags_hi: u8,
    qdcount: u16,
    ancount: u16,
    nscount: u16,
    arcount: u16,
}

impl Header {
    fn read(raw: &[u8]) -> Result<Self, WireError> {
        if raw.len() < HEADER_LEN {
            return Err(WireError("shorter than a header"));
        }
        let word = |at: usize| u16::from_be_bytes([raw[at], raw[at + 1]]);
        Ok(Self {
            id: word(0),
            flags_hi: raw[2],
            qdcount: word(4),
            ancount: word(6),
            nscount: word(8),
            arcount: word(10),
        })
    }
}

fn read_u16(raw: &[u8], at: usize) -> Result<u16, WireError> {
    raw.get(at..at + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .ok_or(WireError("truncated field"))
}

fn read_u32(raw: &[u8], at: usize) -> Result<u32, WireError> {
    raw.get(at..at + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(WireError("truncated field"))
}

/// Reads a possibly compressed name at `start`; returns it and the offset just after it.
fn read_name(raw: &[u8], start: usize) -> Result<(String, usize), WireError> {
    let mut labels: Vec<String> = Vec::new();
    let mut pos = start;
    let mut end = None;
    let mut jumps = 0;
    let mut length = 0;
    loop {
        let len = *raw.get(pos).ok_or(WireError("truncated name"))?;
        match len & 0xc0 {
            0xc0 => {
                let low = *raw.get(pos + 1).ok_or(WireError("truncated pointer"))?;
                end.get_or_insert(pos + 2);
                jumps += 1;
                if jumps > 64 {
                    return Err(WireError("compression loop"));
                }
                pos = (usize::from(len & 0x3f) << 8) | usize::from(low);
            }
            0x00 if len == 0 => {
                end.get_or_insert(pos + 1);
                break;
            }
            0x00 => {
                let label = raw
                    .get(pos + 1..pos + 1 + usize::from(len))
                    .ok_or(WireError("truncated label"))?;
                length += label.len() + 1;
                if length > 255 {
                    return Err(WireError("name too long"));
                }
                labels.push(String::from_utf8_lossy(label).into_owned());
                pos += 1 + usize::from(len);
            }
            _ => return Err(WireError("unknown label type")),
        }
    }
    Ok((labels.join("."), end.expect("set before break")))
}

fn read_question(raw: &[u8], start: usize) -> Result<(Question, usize), WireError> {
    let (name, pos) = read_name(raw, start)?;
    let question = Question {
        name,
        qtype: read_u16(raw, pos)?,
        qclass: read_u16(raw, pos + 2)?,
    };
    Ok((question, pos + 4))
}

fn read_record(raw: &[u8], start: usize) -> Result<(Record, usize), WireError> {
    let (name, pos) = read_name(raw, start)?;
    let rdata_len = usize::from(read_u16(raw, pos + 8)?);
    let rdata_offset = pos + 10;
    if raw.len() < rdata_offset + rdata_len {
        return Err(WireError("truncated record data"));
    }
    let record = Record {
        name,
        rtype: read_u16(raw, pos)?,
        class: read_u16(raw, pos + 2)?,
        ttl: read_u32(raw, pos + 4)?,
        ttl_offset: pos + 4,
        rdata_offset,
        rdata_len,
    };
    Ok((record, rdata_offset + rdata_len))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dns::ResponseBuilder;

    fn query(name: &str, qtype: u16) -> DnsQuery {
        DnsQuery::parse(&DnsQuery::build(0x1234, name, qtype)).unwrap()
    }

    #[test]
    fn round_trips_a_built_query() {
        let q = query("WWW.Example.com", TYPE_A);
        assert_eq!(q.id, 0x1234);
        assert_eq!(q.question.name, "WWW.Example.com");
        assert_eq!((q.question.qtype, q.question.qclass), (TYPE_A, CLASS_IN));
        assert_eq!(q.udp_limit(), 512);
    }

    #[test]
    fn reads_the_edns_udp_size() {
        let mut raw = DnsQuery::build(1, "example.com", TYPE_A);
        raw[11] = 1; // ARCOUNT = 1
        raw.extend_from_slice(&[0, 0, 41, 0x10, 0x00, 0, 0, 0, 0, 0, 0]);
        assert_eq!(DnsQuery::parse(&raw).unwrap().udp_limit(), 4096);
    }

    #[test]
    fn rejects_garbage() {
        assert!(DnsQuery::parse(&[1, 2, 3]).is_err());
        let mut raw = DnsQuery::build(1, "example.com", TYPE_A);
        raw.truncate(raw.len() - 3);
        assert!(DnsQuery::parse(&raw).is_err());
        let mut looped = DnsQuery::build(1, "example.com", TYPE_A);
        looped.truncate(HEADER_LEN);
        looped.extend_from_slice(&[0xc0, 12, 0, 1, 0, 1]);
        assert!(DnsQuery::parse(&looped).is_err());
    }

    #[test]
    fn empty_and_truncated_replies_echo_the_question() {
        let q = query("example.com", TYPE_AAAA);
        let reply = DnsResponse::parse(q.reply(RCODE_SERVFAIL)).unwrap();
        assert_eq!(reply.id(), 0x1234);
        assert_eq!(reply.rcode(), RCODE_SERVFAIL);
        assert!(reply.answers().is_empty());
        assert!(!reply.truncated());
        assert!(DnsResponse::parse(q.truncated_reply()).unwrap().truncated());
    }

    #[test]
    fn follows_cname_chains_with_compressed_names() {
        let q = query("www.example.com", TYPE_A);
        let raw = ResponseBuilder::new(&q)
            .cname("www.example.com", "edge.cdn.net", 300)
            .cname("edge.cdn.net", "e1.cdn.net", 60)
            .a("e1.cdn.net", Ipv4Addr::new(34, 1, 2, 3), 20)
            .a("e1.cdn.net", Ipv4Addr::new(34, 1, 2, 4), 20)
            .a("unrelated.net", Ipv4Addr::new(9, 9, 9, 9), 20)
            .build();
        let response = DnsResponse::parse(raw).unwrap();
        assert_eq!(
            response.ipv4_chain("WWW.example.com."),
            [Ipv4Addr::new(34, 1, 2, 3), Ipv4Addr::new(34, 1, 2, 4)]
        );
        assert_eq!(response.min_answer_ttl(), Some(20));
    }

    #[test]
    fn patches_ids_and_ttls_in_place() {
        let q = query("example.com", TYPE_A);
        let raw = ResponseBuilder::new(&q)
            .a("example.com", Ipv4Addr::new(1, 2, 3, 4), 86_400)
            .a("example.com", Ipv4Addr::new(1, 2, 3, 5), 30)
            .build();
        let mut response = DnsResponse::parse(raw).unwrap();
        response.cap_ttls(300);
        response.set_id(7);
        let reparsed = DnsResponse::parse(response.into_bytes()).unwrap();
        assert_eq!(reparsed.id(), 7);
        let ttls: Vec<u32> = reparsed.answers().iter().map(|r| r.ttl).collect();
        assert_eq!(ttls, [300, 30]);
    }

    #[test]
    fn drops_held_addresses_and_keeps_the_rest_of_the_chain() {
        let q = query("app.example.com", TYPE_A);
        let mut raw = ResponseBuilder::new(&q)
            .cname("app.example.com", "edge.cdn.net", 300)
            .a("edge.cdn.net", Ipv4Addr::new(192, 0, 2, 93), 60)
            .a("edge.cdn.net", Ipv4Addr::new(192, 0, 2, 94), 60)
            .build();
        raw[11] = 1; // ARCOUNT = 1: an OPT record
        raw.extend_from_slice(&[0, 0, 41, 0x10, 0x00, 0, 0, 0, 0, 0, 0]);
        let response = DnsResponse::parse(raw).unwrap();
        let held = Ipv4Addr::new(192, 0, 2, 93);
        let kept = response.without_addresses(&[held]).unwrap();
        assert_eq!(kept.id(), 0x1234);
        assert_eq!(
            kept.ipv4_chain("app.example.com"),
            [Ipv4Addr::new(192, 0, 2, 94)]
        );
        assert_eq!(kept.answers().len(), 2);
        assert_eq!(kept.answers()[1].ttl, 60);
        assert_eq!(kept.records.last().unwrap().rtype, TYPE_OPT);
    }
}
