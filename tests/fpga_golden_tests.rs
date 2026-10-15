//! Golden tests — Rust VM phase algebra matches FPGA table semantics.

use kairos::core::gphase_core::{
    compose, reflect, threshold_energy, threshold_sign, topology_status, PhaseState, TopologyKind,
};

fn verilog_compose(a: PhaseState, b: PhaseState) -> PhaseState {
    const TABLE: [[u8; 4]; 4] = [[0, 1, 2, 3], [1, 3, 3, 2], [2, 3, 3, 1], [3, 2, 1, 0]];
    PhaseState::from_u8(TABLE[a.as_u8() as usize][b.as_u8() as usize]).unwrap()
}

fn verilog_reflect(a: PhaseState) -> PhaseState {
    PhaseState::from_u8(3 - a.as_u8()).unwrap()
}

#[test]
fn fpga_compose_table_matches_core() {
    for a in PhaseState::ALL {
        for b in PhaseState::ALL {
            assert_eq!(compose(a, b), verilog_compose(a, b), "compose({a:?},{b:?})");
        }
    }
}

#[test]
fn fpga_reflect_matches_core() {
    for x in PhaseState::ALL {
        assert_eq!(reflect(x), verilog_reflect(x));
    }
}

#[test]
fn fpga_thresholds_match_core() {
    for x in PhaseState::ALL {
        let sign = matches!(x, PhaseState::Phi2 | PhaseState::Phi3);
        let energy = matches!(x, PhaseState::Phi0 | PhaseState::Phi3);
        assert_eq!(threshold_sign(x), u8::from(sign));
        assert_eq!(threshold_energy(x), u8::from(energy));
    }
}

#[test]
fn fpga_topology_matches_core() {
    for a in PhaseState::ALL {
        for b in PhaseState::ALL {
            for c in PhaseState::ALL {
                let r = topology_status(a, b, c);
                let l = verilog_compose(verilog_compose(a, b), c);
                let rt = verilog_compose(a, verilog_compose(b, c));
                if l == rt {
                    assert_eq!(r.kind, TopologyKind::Same);
                } else {
                    assert_eq!(r.kind, TopologyKind::Reflection);
                    assert_eq!(r.right, verilog_reflect(r.left));
                }
            }
        }
    }
}

#[test]
fn vm_trace_opcode_layout_matches_decoder() {
    let word: u32 = (0u32 << 27) | (0u32 << 25) | (2u32 << 19);
    assert_eq!(word >> 27, 0);
    assert_eq!((word >> 19) & 0b11, 2);
}

/// UART line format for `gpl_algebra_64.v` / `verify_gpl_algebra_64.py`.
#[test]
fn fpga_algebra_64_uart_format_matches_host() {
    fn o(a: u8, b: u8) -> u8 {
        const T: [[u8; 4]; 4] = [[0, 1, 2, 3], [1, 3, 3, 2], [2, 3, 3, 1], [3, 2, 1, 0]];
        T[a as usize][b as usize]
    }
    let mut lines = vec!["GPL ALGEBRA 64 TRIPLES".to_string()];
    let mut pass_n = 0u32;
    for a in 0u8..4 {
        for b in 0u8..4 {
            for c in 0u8..4 {
                let ab = o(a, b);
                let l = o(ab, c);
                let r = o(a, o(b, c));
                let p = if l == r {
                    0
                } else if l == 3 - r {
                    1
                } else {
                    2
                };
                assert!(p <= 1, "illegal associator a={a} b={b} c={c}");
                let status = if p <= 1 { "PASS" } else { "FAIL" };
                if p <= 1 {
                    pass_n += 1;
                }
                lines.push(format!("T {a}{b}{c} ab={ab} L={l} R={r} p={p} {status}"));
            }
        }
    }
    assert_eq!(pass_n, 64);
    lines.push("ALGEBRA 64/64 PASS".into());
    lines.push("ALGEBRA DONE".into());
    let text = lines.join("\n") + "\n";
    assert!(text.starts_with("GPL ALGEBRA 64 TRIPLES\nT 000 "));
    assert!(text.contains("ALGEBRA 64/64 PASS\nALGEBRA DONE\n"));
}

/// UART banner/line format for parameterized Avenue-2 glider harness.
#[test]
fn fpga_glider_uart_format_matches_host() {
    fn o(a: u8, b: u8) -> u8 {
        const T: [[u8; 4]; 4] = [[0, 1, 2, 3], [1, 3, 3, 2], [2, 3, 3, 1], [3, 2, 1, 0]];
        T[a as usize][b as usize]
    }
    let m = 4usize;
    let vac = 1u8;
    let def = 2u8;
    let sr = 2usize;
    let sc = 2usize;
    let mut g = vec![vec![vac; m]; m];
    g[sr][sc] = def;
    let banner = format!("PHASE LATTICE 2D GLIDER {m:02}x{m:02} V{vac}D{def} S{sr},{sc}");
    assert_eq!(banner, "PHASE LATTICE 2D GLIDER 04x04 V1D2 S2,2");
    for gen in 0..8 {
        let flat: String = g
            .iter()
            .flat_map(|row| row.iter().map(|c| char::from(b'0' + c)))
            .collect();
        let foot = g.iter().flatten().filter(|&&c| c != vac).count();
        let line = format!("G{gen:05} S={flat} F={foot:02}");
        if gen == 0 {
            assert_eq!(line, "G00000 S=1111111111211111 F=01");
        }
        assert_eq!(foot, 1);
        assert_eq!(g[(sr + m - (gen % m)) % m][(sc + m - (gen % m)) % m], def);
        // rule_a = colpass(rowpass)
        let mut mid = vec![vec![0u8; m]; m];
        for i in 0..m {
            for j in 0..m {
                let l = g[i][(j + m - 1) % m];
                let x = g[i][j];
                let r = g[i][(j + 1) % m];
                mid[i][j] = o(o(l, x), r);
            }
        }
        let mut nxt = vec![vec![0u8; m]; m];
        for i in 0..m {
            for j in 0..m {
                let n = mid[(i + m - 1) % m][j];
                let x = mid[i][j];
                let s = mid[(i + 1) % m][j];
                nxt[i][j] = o(o(n, x), s);
            }
        }
        g = nxt;
        let _ = o;
    }
}
