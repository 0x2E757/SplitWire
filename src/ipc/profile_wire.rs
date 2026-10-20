use super::{Reader, WireError, Writer};
use crate::profile::{
    ChangeRow, CheckLevel, ConfLine, Endpoint, ImportCheck, ImportPreview, LineOutcome, Pin,
    TunnelAddress,
};

/// The wire form of the profile types the messages reuse. None of them holds key material.
pub struct ProfileWire;

impl ProfileWire {
    const CHECK_LEVELS: [CheckLevel; 4] = [
        CheckLevel::Ok,
        CheckLevel::Info,
        CheckLevel::Warn,
        CheckLevel::Bad,
    ];
    const OUTCOMES: [LineOutcome; 3] = [
        LineOutcome::Read,
        LineOutcome::Ignored,
        LineOutcome::Problem,
    ];

    pub fn pin(pin: &Pin, w: &mut Writer) {
        w.u64(pin.number().into());
    }

    pub fn read_pin(r: &mut Reader) -> Result<Pin, WireError> {
        Pin::new(r.u8()?).ok_or(WireError::Invalid("a colour other than 1-6"))
    }

    pub fn address(address: &TunnelAddress, w: &mut Writer) {
        w.ipv4(address.address);
        w.u64(address.prefix.into());
    }

    pub fn read_address(r: &mut Reader) -> Result<TunnelAddress, WireError> {
        let text = format!("{}/{}", r.ipv4()?, r.u8()?);
        TunnelAddress::parse(&text).map_err(|_| WireError::Invalid("a bad tunnel address"))
    }

    pub fn endpoint(endpoint: &Endpoint, w: &mut Writer) {
        w.str(&endpoint.to_string());
    }

    pub fn read_endpoint(r: &mut Reader) -> Result<Endpoint, WireError> {
        Endpoint::parse(r.str()?).map_err(|_| WireError::Invalid("a bad endpoint"))
    }

    pub fn preview(preview: &ImportPreview, w: &mut Writer) {
        w.str(&preview.name);
        w.str(&preview.file_name);
        w.bool(preview.replace);
        w.opt_str(preview.name_problem.as_ref());
        w.list(&preview.values, |(field, value), w| {
            w.str(field);
            w.str(value);
        });
        w.list(&preview.changes, Self::change);
        w.list(&preview.checks, Self::check);
        w.list(&preview.lines, Self::conf_line);
        Self::pin(&preview.colour, w);
    }

    pub fn read_preview(r: &mut Reader) -> Result<ImportPreview, WireError> {
        Ok(ImportPreview {
            name: r.string()?,
            file_name: r.string()?,
            replace: r.bool()?,
            name_problem: r.opt_string()?,
            values: r.list(|r| Ok((r.string()?, r.string()?)))?,
            changes: r.list(Self::read_change)?,
            checks: r.list(Self::read_check)?,
            lines: r.list(Self::read_conf_line)?,
            colour: Self::read_pin(r)?,
        })
    }

    fn change(row: &ChangeRow, w: &mut Writer) {
        w.str(&row.field);
        w.str(&row.old);
        w.str(&row.new);
    }

    fn read_change(r: &mut Reader) -> Result<ChangeRow, WireError> {
        Ok(ChangeRow {
            field: r.string()?,
            old: r.string()?,
            new: r.string()?,
        })
    }

    fn check(check: &ImportCheck, w: &mut Writer) {
        w.choice(&check.level, &Self::CHECK_LEVELS);
        w.str(&check.title);
        w.str(&check.detail);
    }

    fn read_check(r: &mut Reader) -> Result<ImportCheck, WireError> {
        Ok(ImportCheck {
            level: r.choice(&Self::CHECK_LEVELS)?,
            title: r.string()?,
            detail: r.string()?,
        })
    }

    fn conf_line(line: &ConfLine, w: &mut Writer) {
        w.usize(line.line);
        w.str(&line.key);
        w.str(&line.shown);
        w.choice(&line.outcome, &Self::OUTCOMES);
        w.str(&line.reason);
    }

    fn read_conf_line(r: &mut Reader) -> Result<ConfLine, WireError> {
        Ok(ConfLine {
            line: r.usize()?,
            key: r.string()?,
            shown: r.string()?,
            outcome: r.choice(&Self::OUTCOMES)?,
            reason: r.string()?,
        })
    }
}
