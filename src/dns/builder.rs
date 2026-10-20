use std::net::Ipv4Addr;

use super::wire::{CLASS_IN, DnsQuery, RCODE_NOERROR, TYPE_A, TYPE_CNAME, write_name};

/// Builds answers to a query; what the fake DNS servers of the tests and smoke checks reply.
pub struct ResponseBuilder {
    query: DnsQuery,
    rcode: u8,
    answers: Vec<(String, u16, u32, Vec<u8>)>,
}

impl ResponseBuilder {
    pub fn new(query: &DnsQuery) -> Self {
        Self {
            query: query.clone(),
            rcode: RCODE_NOERROR,
            answers: Vec::new(),
        }
    }

    pub fn rcode(mut self, rcode: u8) -> Self {
        self.rcode = rcode;
        self
    }

    pub fn a(mut self, name: &str, ip: Ipv4Addr, ttl: u32) -> Self {
        self.answers
            .push((name.to_owned(), TYPE_A, ttl, ip.octets().to_vec()));
        self
    }

    pub fn cname(mut self, name: &str, target: &str, ttl: u32) -> Self {
        let mut rdata = Vec::new();
        write_name(&mut rdata, target);
        self.answers.push((name.to_owned(), TYPE_CNAME, ttl, rdata));
        self
    }

    pub fn build(self) -> Vec<u8> {
        let mut out = self.query.reply(self.rcode);
        out[6..8].copy_from_slice(&(self.answers.len() as u16).to_be_bytes());
        for (name, rtype, ttl, rdata) in &self.answers {
            if name.eq_ignore_ascii_case(&self.query.question.name) {
                out.extend_from_slice(&[0xc0, 12]); // compressed: points at the question
            } else {
                write_name(&mut out, name);
            }
            out.extend_from_slice(&rtype.to_be_bytes());
            out.extend_from_slice(&CLASS_IN.to_be_bytes());
            out.extend_from_slice(&ttl.to_be_bytes());
            out.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
            out.extend_from_slice(rdata);
        }
        out
    }
}
