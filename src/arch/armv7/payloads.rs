//! Embedded A32 isolation payloads. Not ELF. Real ARM instructions.

pub const SYS_EXIT: u32 = 1;
pub const SYS_WRITE: u32 = 2;
pub const LEGAL_MAGIC: u32 = 0x4C454741; // "LEGA"

fn w(words: &[u32]) -> [u8; 64] {
    let mut out = [0u8; 64];
    for (i, w) in words.iter().enumerate() {
        let b = w.to_le_bytes();
        let o = i * 4;
        out[o..o + 4].copy_from_slice(&b);
    }
    out
}

/// ldr r2,[r0]; cmp r2,r1; bne fail; add r0,#8; mov r1,#9; mov r7,#2; svc; mov r0,#0; mov r7,#1; svc; fail: mov r0,#1; mov r7,#1; svc
pub fn legal_read() -> [u8; 64] {
    w(&[
        0xE590_2000,
        0xE152_0001,
        0x1A00_0006,
        0xE280_0008,
        0xE3A0_1009,
        0xE3A0_7002,
        0xEF00_0000,
        0xE3A0_0000,
        0xE3A0_7001,
        0xEF00_0000,
        0xE3A0_0001,
        0xE3A0_7001,
        0xEF00_0000,
    ])
}

pub fn kernel_read() -> [u8; 64] {
    w(&[
        0xE300_0000, // movw r0, #0
        0xE340_0010, // movt r0, #0x0010
        0xE590_1000, // ldr r1, [r0]
        0xE3A0_0001,
        0xE3A0_7001,
        0xEF00_0000,
    ])
}

pub fn kernel_write() -> [u8; 64] {
    w(&[
        0xE300_0000,
        0xE340_0010,
        0xE3A0_1000,
        0xE580_1000, // str r1, [r0]
        0xE3A0_0001,
        0xE3A0_7001,
        0xEF00_0000,
    ])
}

pub fn peer_read() -> [u8; 64] {
    w(&[
        0xE590_1000, // ldr r1, [r0]  r0 = peer data from kernel
        0xE3A0_0001,
        0xE3A0_7001,
        0xEF00_0000,
    ])
}

pub fn peer_write() -> [u8; 64] {
    w(&[
        0xE3A0_1000,
        0xE580_1000,
        0xE3A0_0001,
        0xE3A0_7001,
        0xEF00_0000,
    ])
}

pub fn mmio_write() -> [u8; 64] {
    w(&[
        0xE300_0000, // movw r0,#0
        0xE34E_0000, // movt r0,#0xE000
        0xE3A0_1058, // mov r1, #'X'
        0xE580_1030, // str r1, [r0, #0x30]
        0xE3A0_0001,
        0xE3A0_7001,
        0xEF00_0000,
    ])
}

/// Goal 5 negative control: write to the payload's own code page, which is
/// mapped `user_rx_page` (`PrivRoUserRo`). Must take a write permission fault.
///
/// `sub r0, pc, #8` leaves `r0` holding the address of that very instruction
/// (A32 reads PC as current + 8), so the payload finds its own page without
/// the kernel passing it in.
///
/// sub r0,pc,#8; mov r1,#0; str r1,[r0]; mov r0,#1; mov r7,#1; svc
pub fn code_write() -> [u8; 64] {
    w(&[
        0xE24F_0008,
        0xE3A0_1000,
        0xE580_1000, // str r1, [r0]  -> permission fault, user code is RO
        0xE3A0_0001,
        0xE3A0_7001,
        0xEF00_0000,
    ])
}

/// Goal 5 positive control: read the payload's own code page, which user mode
/// *is* allowed to do (`PrivRoUserRo`). Must complete and exit cleanly, so a
/// blanket "everything faults" cannot masquerade as isolation.
///
/// sub r0,pc,#8; ldr r1,[r0]; mov r0,#0; mov r7,#1; svc
pub fn code_read() -> [u8; 64] {
    w(&[
        0xE24F_0008,
        0xE590_1000, // ldr r1, [r0]  -> permitted
        0xE3A0_0000,
        0xE3A0_7001,
        0xEF00_0000,
    ])
}

pub fn execute_data() -> [u8; 64] {
    w(&[
        0xE12F_FF10, // bx r0  (r0 = data page)
    ])
}

pub fn stack_overflow() -> [u8; 64] {
    w(&[
        0xE53D_1004, // ldr r1, [sp, #-4]!
        0xEAFF_FFFD, // b loop
    ])
}

pub fn legal_message() -> &'static [u8] {
    b"LEGAL_OK\n"
}

pub struct NamedPayload {
    pub name: &'static str,
    pub code: [u8; 64],
    pub expect_fault: bool,
    pub needs_peer: bool,
    pub data_is_pc: bool,
}

pub fn matrix() -> [NamedPayload; 10] {
    [
        NamedPayload {
            name: "legal-read",
            code: legal_read(),
            expect_fault: false,
            needs_peer: false,
            data_is_pc: false,
        },
        NamedPayload {
            name: "code-read",
            code: code_read(),
            expect_fault: false,
            needs_peer: false,
            data_is_pc: false,
        },
        NamedPayload {
            name: "kernel-read",
            code: kernel_read(),
            expect_fault: true,
            needs_peer: false,
            data_is_pc: false,
        },
        NamedPayload {
            name: "kernel-write",
            code: kernel_write(),
            expect_fault: true,
            needs_peer: false,
            data_is_pc: false,
        },
        NamedPayload {
            name: "peer-read",
            code: peer_read(),
            expect_fault: true,
            needs_peer: true,
            data_is_pc: false,
        },
        NamedPayload {
            name: "peer-write",
            code: peer_write(),
            expect_fault: true,
            needs_peer: true,
            data_is_pc: false,
        },
        NamedPayload {
            name: "mmio-write",
            code: mmio_write(),
            expect_fault: true,
            needs_peer: false,
            data_is_pc: false,
        },
        NamedPayload {
            name: "code-write",
            code: code_write(),
            expect_fault: true,
            needs_peer: false,
            data_is_pc: false,
        },
        NamedPayload {
            name: "execute-data",
            code: execute_data(),
            expect_fault: true,
            needs_peer: false,
            data_is_pc: true,
        },
        NamedPayload {
            name: "stack-overflow",
            code: stack_overflow(),
            expect_fault: true,
            needs_peer: false,
            data_is_pc: false,
        },
    ]
}
