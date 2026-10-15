//! Compare device dumps against frozen GPLV1 vectors.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use super::decode::decode_oracle;
use super::format::{OracleRecord, RecordKind};

#[derive(Debug)]
pub enum DeviceDumpError {
    Io(io::Error),
    Oracle(String),
    Parse { line: usize, msg: String },
}

impl From<io::Error> for DeviceDumpError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl std::fmt::Display for DeviceDumpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Oracle(e) => write!(f, "oracle: {e}"),
            Self::Parse { line, msg } => write!(f, "parse line {line}: {msg}"),
        }
    }
}

impl std::error::Error for DeviceDumpError {}

#[derive(Debug, Clone, Default)]
pub struct CompareReport {
    pub compose_pass: u32,
    pub compose_total: u32,
    pub reflect_pass: u32,
    pub reflect_total: u32,
    pub algebra_pass: u32,
    pub algebra_total: u32,
    pub parity_illegal: u32,
    pub mismatch_count: u32,
    pub first_failure: Option<String>,
    pub ok: bool,
}

impl CompareReport {
    pub fn token_block(&self) -> String {
        format!(
            "COMPOSE {}/{} {}\n\
REFLECT {}/{} {}\n\
ALGEBRA {}/{} {}\n\
PARITY_ILLEGAL={}\n\
MISMATCH_COUNT={}",
            self.compose_pass,
            self.compose_total,
            if self.compose_pass == self.compose_total && self.compose_total > 0 {
                "PASS"
            } else {
                "FAIL"
            },
            self.reflect_pass,
            self.reflect_total,
            if self.reflect_pass == self.reflect_total && self.reflect_total > 0 {
                "PASS"
            } else {
                "FAIL"
            },
            self.algebra_pass,
            self.algebra_total,
            if self.algebra_pass == self.algebra_total && self.algebra_total > 0 {
                "PASS"
            } else {
                "FAIL"
            },
            self.parity_illegal,
            self.mismatch_count,
        )
    }
}

#[derive(Debug, Clone)]
enum DumpLine {
    Compose {
        seq: u32,
        a: u8,
        b: u8,
        compose_ab: u8,
    },
    Reflect {
        seq: u32,
        a: u8,
        rho_a: u8,
    },
    Triple {
        seq: u32,
        a: u8,
        b: u8,
        c: u8,
        ab: u8,
        left: u8,
        right: u8,
        parity: u8,
    },
}

fn parse_u8(tok: &str, line: usize, name: &str) -> Result<u8, DeviceDumpError> {
    tok.parse::<u8>().map_err(|_| DeviceDumpError::Parse {
        line,
        msg: format!("bad {name}"),
    })
}

fn parse_u32(tok: &str, line: usize, name: &str) -> Result<u32, DeviceDumpError> {
    tok.parse::<u32>().map_err(|_| DeviceDumpError::Parse {
        line,
        msg: format!("bad {name}"),
    })
}

fn parse_dump(text: &str) -> Result<Vec<DumpLine>, DeviceDumpError> {
    let mut out = Vec::new();
    for (idx, raw) in text.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        match parts[0] {
            "C" => {
                if parts.len() != 5 {
                    return Err(DeviceDumpError::Parse {
                        line: line_no,
                        msg: "C expects: C <seq> <a> <b> <compose_ab>".into(),
                    });
                }
                out.push(DumpLine::Compose {
                    seq: parse_u32(parts[1], line_no, "seq")?,
                    a: parse_u8(parts[2], line_no, "a")?,
                    b: parse_u8(parts[3], line_no, "b")?,
                    compose_ab: parse_u8(parts[4], line_no, "compose_ab")?,
                });
            }
            "R" => {
                if parts.len() != 4 {
                    return Err(DeviceDumpError::Parse {
                        line: line_no,
                        msg: "R expects: R <seq> <a> <rho_a>".into(),
                    });
                }
                out.push(DumpLine::Reflect {
                    seq: parse_u32(parts[1], line_no, "seq")?,
                    a: parse_u8(parts[2], line_no, "a")?,
                    rho_a: parse_u8(parts[3], line_no, "rho_a")?,
                });
            }
            "T" => {
                if parts.len() != 9 {
                    return Err(DeviceDumpError::Parse {
                        line: line_no,
                        msg: "T expects: T <seq> <a> <b> <c> <ab> <left> <right> <parity>".into(),
                    });
                }
                out.push(DumpLine::Triple {
                    seq: parse_u32(parts[1], line_no, "seq")?,
                    a: parse_u8(parts[2], line_no, "a")?,
                    b: parse_u8(parts[3], line_no, "b")?,
                    c: parse_u8(parts[4], line_no, "c")?,
                    ab: parse_u8(parts[5], line_no, "ab")?,
                    left: parse_u8(parts[6], line_no, "left")?,
                    right: parse_u8(parts[7], line_no, "right")?,
                    parity: parse_u8(parts[8], line_no, "parity")?,
                });
            }
            _ => {
                return Err(DeviceDumpError::Parse {
                    line: line_no,
                    msg: format!("unknown record kind token {}", parts[0]),
                });
            }
        }
    }
    Ok(out)
}

fn fail(report: &mut CompareReport, msg: String) {
    report.mismatch_count += 1;
    report.ok = false;
    if report.first_failure.is_none() {
        report.first_failure = Some(msg);
    }
}

pub fn compare_device_dump_str(
    oracle_blob: &[u8],
    dump_text: &str,
) -> Result<CompareReport, DeviceDumpError> {
    let (_hdr, records) =
        decode_oracle(oracle_blob).map_err(|e| DeviceDumpError::Oracle(e.to_string()))?;
    let by_seq: HashMap<u32, &OracleRecord> = records.iter().map(|r| (r.sequence, r)).collect();

    let mut report = CompareReport {
        compose_total: records
            .iter()
            .filter(|r| r.kind == RecordKind::Compose)
            .count() as u32,
        reflect_total: records
            .iter()
            .filter(|r| r.kind == RecordKind::Reflection)
            .count() as u32,
        algebra_total: records
            .iter()
            .filter(|r| r.kind == RecordKind::Triple)
            .count() as u32,
        ok: true,
        ..Default::default()
    };

    let dump = parse_dump(dump_text)?;
    let mut seen: HashMap<u32, usize> = HashMap::new();
    let expected_order: Vec<u32> = records.iter().map(|r| r.sequence).collect();
    let mut dump_order: Vec<u32> = Vec::new();

    for line in &dump {
        let seq = match line {
            DumpLine::Compose { seq, .. }
            | DumpLine::Reflect { seq, .. }
            | DumpLine::Triple { seq, .. } => *seq,
        };
        dump_order.push(seq);
        *seen.entry(seq).or_insert(0) += 1;
        if seen[&seq] > 1 {
            fail(&mut report, format!("duplicate record sequence={seq}"));
            continue;
        }
        let Some(exp) = by_seq.get(&seq) else {
            fail(
                &mut report,
                format!("unexpected sequence={seq} not in oracle"),
            );
            continue;
        };

        match (line, exp.kind) {
            (
                DumpLine::Compose {
                    a, b, compose_ab, ..
                },
                RecordKind::Compose,
            ) => {
                if *a > 3 || *b > 3 || *compose_ab > 3 {
                    fail(
                        &mut report,
                        format!("malformed state compose seq={seq} a={a} b={b} y={compose_ab}"),
                    );
                    continue;
                }
                if *a != exp.input_a || *b != exp.input_b || *compose_ab != exp.compose_ab {
                    fail(
                        &mut report,
                        format!(
                            "compose mismatch seq={seq} kind=1 inputs=({},{}) \
                             expected={} actual={}",
                            exp.input_a, exp.input_b, exp.compose_ab, compose_ab
                        ),
                    );
                } else {
                    report.compose_pass += 1;
                }
            }
            (DumpLine::Reflect { a, rho_a, .. }, RecordKind::Reflection) => {
                if *a > 3 || *rho_a > 3 {
                    fail(
                        &mut report,
                        format!("malformed state reflect seq={seq} a={a} rho={rho_a}"),
                    );
                    continue;
                }
                if *a != exp.input_a || *rho_a != exp.rho_a {
                    fail(
                        &mut report,
                        format!(
                            "reflect mismatch seq={seq} kind=2 input={} expected={} actual={}",
                            exp.input_a, exp.rho_a, rho_a
                        ),
                    );
                } else {
                    report.reflect_pass += 1;
                }
            }
            (
                DumpLine::Triple {
                    a,
                    b,
                    c,
                    ab,
                    left,
                    right,
                    parity,
                    ..
                },
                RecordKind::Triple,
            ) => {
                if *parity > 1 {
                    report.parity_illegal += 1;
                    fail(
                        &mut report,
                        format!("illegal parity seq={seq} parity={parity}"),
                    );
                    continue;
                }
                if *a > 3 || *b > 3 || *c > 3 || *ab > 3 || *left > 3 || *right > 3 {
                    fail(&mut report, format!("malformed state triple seq={seq}"));
                    continue;
                }
                if *a != exp.input_a
                    || *b != exp.input_b
                    || *c != exp.input_c
                    || *ab != exp.compose_ab
                    || *left != exp.left_association
                    || *right != exp.right_association
                    || *parity != exp.parity
                {
                    fail(
                        &mut report,
                        format!(
                            "triple mismatch seq={seq} kind=3 inputs=({},{},{}) \
                             expected ab={} L={} R={} p={} \
                             actual ab={} L={} R={} p={}",
                            exp.input_a,
                            exp.input_b,
                            exp.input_c,
                            exp.compose_ab,
                            exp.left_association,
                            exp.right_association,
                            exp.parity,
                            ab,
                            left,
                            right,
                            parity
                        ),
                    );
                } else {
                    report.algebra_pass += 1;
                }
            }
            _ => {
                fail(
                    &mut report,
                    format!(
                        "record kind mismatch sequence={seq} dump vs oracle {:?}",
                        exp.kind
                    ),
                );
            }
        }
    }

    for r in &records {
        if !seen.contains_key(&r.sequence) {
            fail(
                &mut report,
                format!("missing record sequence={} kind={:?}", r.sequence, r.kind),
            );
        }
    }

    // Ordering required: dump sequences must match oracle order for present lines
    // Full suite must be exact permutation of expected order.
    if dump_order.len() == expected_order.len() && dump_order != expected_order {
        fail(
            &mut report,
            format!(
                "reordered sequences: first dump={} first expected={}",
                dump_order.first().copied().unwrap_or(0),
                expected_order.first().copied().unwrap_or(0)
            ),
        );
    }

    if report.compose_pass != report.compose_total
        || report.reflect_pass != report.reflect_total
        || report.algebra_pass != report.algebra_total
        || report.mismatch_count != 0
        || report.parity_illegal != 0
    {
        report.ok = false;
    }

    Ok(report)
}

pub fn compare_device_dump(
    oracle_path: &Path,
    dump_path: &Path,
) -> Result<CompareReport, DeviceDumpError> {
    let blob = fs::read(oracle_path)?;
    let text = fs::read_to_string(dump_path)?;
    compare_device_dump_str(&blob, &text)
}

/// Build a canonical dump from oracle records (for positive tests / sim export).
pub fn dump_from_records(records: &[OracleRecord]) -> String {
    let mut s = String::new();
    for r in records {
        match r.kind {
            RecordKind::Compose => {
                s.push_str(&format!(
                    "C {} {} {} {}\n",
                    r.sequence, r.input_a, r.input_b, r.compose_ab
                ));
            }
            RecordKind::Reflection => {
                s.push_str(&format!("R {} {} {}\n", r.sequence, r.input_a, r.rho_a));
            }
            RecordKind::Triple => {
                s.push_str(&format!(
                    "T {} {} {} {} {} {} {} {}\n",
                    r.sequence,
                    r.input_a,
                    r.input_b,
                    r.input_c,
                    r.compose_ab,
                    r.left_association,
                    r.right_association,
                    r.parity
                ));
            }
        }
    }
    s
}
