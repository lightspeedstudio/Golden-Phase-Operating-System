// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! PZ7020-SL-C PS7 bring-up as named UG585 MMIO steps.
//!
//! Values are this PCB (33.333 MHz crystal, MT41K256M16). This file is not
//! Xilinx FSBL source.

use super::mmio;

#[derive(Clone, Copy)]
enum Step {
    Unlock,
    Lock,
    Write(u32, u32),
    MaskWrite(u32, u32, u32),
    Poll(u32, u32),
    DelayMs(u32),
}

fn run(steps: &[Step]) -> Result<(), u32> {
    unsafe {
        for s in steps {
            match *s {
                Step::Unlock => mmio::slcr_unlock(),
                Step::Lock => mmio::slcr_lock(),
                Step::Write(a, v) => mmio::write(a, v),
                Step::MaskWrite(a, m, v) => mmio::mask_write(a, m, v),
                Step::Poll(a, m) => mmio::poll(a, m)?,
                Step::DelayMs(ms) => mmio::delay_ms(ms),
            }
        }
    }
    Ok(())
}

/// MIO pin mux + SLCR analog (UG585 Ch. 2).
unsafe fn mio() -> Result<(), u32> {
    const STEPS: &[Step] = &[
        Step::Unlock,
        Step::MaskWrite(0xF8000B40, 0x00000FFF, 0x00000600),
        Step::MaskWrite(0xF8000B44, 0x00000FFF, 0x00000600),
        Step::MaskWrite(0xF8000B48, 0x00000FFF, 0x00000672),
        Step::MaskWrite(0xF8000B4C, 0x00000FFF, 0x00000800),
        Step::MaskWrite(0xF8000B50, 0x00000FFF, 0x00000674),
        Step::MaskWrite(0xF8000B54, 0x00000FFF, 0x00000800),
        Step::MaskWrite(0xF8000B58, 0x00000FFF, 0x00000600),
        Step::MaskWrite(0xF8000B5C, 0xFFFFFFFF, 0x0018C068),
        Step::MaskWrite(0xF8000B60, 0xFFFFFFFF, 0x00F98068),
        Step::MaskWrite(0xF8000B64, 0xFFFFFFFF, 0x00F98068),
        Step::MaskWrite(0xF8000B68, 0xFFFFFFFF, 0x00F98068),
        Step::MaskWrite(0xF8000B6C, 0x00007FFF, 0x00000220),
        Step::MaskWrite(0xF8000B70, 0x00000001, 0x00000001),
        Step::MaskWrite(0xF8000B70, 0x00000021, 0x00000020),
        Step::MaskWrite(0xF8000B70, 0x07FEFFFF, 0x00000823),
        Step::MaskWrite(0xF8000700, 0x00003FFF, 0x00001600), // MIO_PIN_00
        Step::MaskWrite(0xF8000704, 0x00003FFF, 0x00001602), // MIO_PIN_01
        Step::MaskWrite(0xF8000708, 0x00003FFF, 0x00000602), // MIO_PIN_02
        Step::MaskWrite(0xF800070C, 0x00003FFF, 0x00000602), // MIO_PIN_03
        Step::MaskWrite(0xF8000710, 0x00003FFF, 0x00000602), // MIO_PIN_04
        Step::MaskWrite(0xF8000714, 0x00003FFF, 0x00000602), // MIO_PIN_05
        Step::MaskWrite(0xF8000718, 0x00003FFF, 0x00000602), // MIO_PIN_06
        Step::MaskWrite(0xF800071C, 0x00003FFF, 0x00000600), // MIO_PIN_07
        Step::MaskWrite(0xF8000720, 0x00003FFF, 0x00000600), // MIO_PIN_08
        Step::MaskWrite(0xF8000724, 0x00003FFF, 0x00001600), // MIO_PIN_09
        Step::MaskWrite(0xF8000728, 0x00003FFF, 0x000016E1), // MIO_PIN_10
        Step::MaskWrite(0xF800072C, 0x00003FFF, 0x000016E0), // MIO_PIN_11
        Step::MaskWrite(0xF8000730, 0x00003FFF, 0x00001600), // MIO_PIN_12
        Step::MaskWrite(0xF8000734, 0x00003FFF, 0x00001600), // MIO_PIN_13
        Step::MaskWrite(0xF8000738, 0x00003FFF, 0x00001600), // MIO_PIN_14
        Step::MaskWrite(0xF800073C, 0x00003FFF, 0x00001600), // MIO_PIN_15
        Step::MaskWrite(0xF8000740, 0x00003FFF, 0x00001202), // MIO_PIN_16
        Step::MaskWrite(0xF8000744, 0x00003FFF, 0x00001202), // MIO_PIN_17
        Step::MaskWrite(0xF8000748, 0x00003FFF, 0x00001202), // MIO_PIN_18
        Step::MaskWrite(0xF800074C, 0x00003FFF, 0x00001202), // MIO_PIN_19
        Step::MaskWrite(0xF8000750, 0x00003FFF, 0x00001202), // MIO_PIN_20
        Step::MaskWrite(0xF8000754, 0x00003FFF, 0x00001202), // MIO_PIN_21
        Step::MaskWrite(0xF8000758, 0x00003FFF, 0x00001203), // MIO_PIN_22
        Step::MaskWrite(0xF800075C, 0x00003FFF, 0x00001203), // MIO_PIN_23
        Step::MaskWrite(0xF8000760, 0x00003FFF, 0x00001203), // MIO_PIN_24
        Step::MaskWrite(0xF8000764, 0x00003FFF, 0x00001203), // MIO_PIN_25
        Step::MaskWrite(0xF8000768, 0x00003FFF, 0x00001203), // MIO_PIN_26
        Step::MaskWrite(0xF800076C, 0x00003FFF, 0x00001203), // MIO_PIN_27
        Step::MaskWrite(0xF8000770, 0x00003FFF, 0x00001204), // MIO_PIN_28
        Step::MaskWrite(0xF8000774, 0x00003FFF, 0x00001205), // MIO_PIN_29
        Step::MaskWrite(0xF8000778, 0x00003FFF, 0x00001204), // MIO_PIN_30
        Step::MaskWrite(0xF800077C, 0x00003FFF, 0x00001205), // MIO_PIN_31
        Step::MaskWrite(0xF8000780, 0x00003FFF, 0x00001204), // MIO_PIN_32
        Step::MaskWrite(0xF8000784, 0x00003FFF, 0x00001204), // MIO_PIN_33
        Step::MaskWrite(0xF8000788, 0x00003FFF, 0x00001204), // MIO_PIN_34
        Step::MaskWrite(0xF800078C, 0x00003FFF, 0x00001204), // MIO_PIN_35
        Step::MaskWrite(0xF8000790, 0x00003FFF, 0x00001205), // MIO_PIN_36
        Step::MaskWrite(0xF8000794, 0x00003FFF, 0x00001204), // MIO_PIN_37
        Step::MaskWrite(0xF8000798, 0x00003FFF, 0x00001204), // MIO_PIN_38
        Step::MaskWrite(0xF800079C, 0x00003FFF, 0x00001204), // MIO_PIN_39
        Step::MaskWrite(0xF80007A0, 0x00003FFF, 0x00001280), // MIO_PIN_40
        Step::MaskWrite(0xF80007A4, 0x00003FFF, 0x00001280), // MIO_PIN_41
        Step::MaskWrite(0xF80007A8, 0x00003FFF, 0x00001280), // MIO_PIN_42
        Step::MaskWrite(0xF80007AC, 0x00003FFF, 0x00001280), // MIO_PIN_43
        Step::MaskWrite(0xF80007B0, 0x00003FFF, 0x00001280), // MIO_PIN_44
        Step::MaskWrite(0xF80007B4, 0x00003FFF, 0x00001280), // MIO_PIN_45
        Step::MaskWrite(0xF80007B8, 0x00003FFF, 0x00001200), // MIO_PIN_46
        Step::MaskWrite(0xF80007BC, 0x00003FFF, 0x00001200), // MIO_PIN_47
        Step::MaskWrite(0xF80007C0, 0x00003FFF, 0x00001200), // MIO_PIN_48
        Step::MaskWrite(0xF80007C4, 0x00003FFF, 0x00001200), // MIO_PIN_49
        Step::MaskWrite(0xF80007C8, 0x00003FFF, 0x00001200), // MIO_PIN_50
        Step::MaskWrite(0xF80007CC, 0x00003FFF, 0x00001200), // MIO_PIN_51
        Step::MaskWrite(0xF80007D0, 0x00003FFF, 0x00001280), // MIO_PIN_52
        Step::MaskWrite(0xF80007D4, 0x00003FFF, 0x00001280), // MIO_PIN_53
        Step::MaskWrite(0xF8000830, 0x003F003F, 0x00380037),
        Step::Lock,
    ];
    run(STEPS)
}

/// ARM/DDR/IO PLL: bypass, FBDIV, reset, lock, un-bypass (UG585).
unsafe fn pll() -> Result<(), u32> {
    const STEPS: &[Step] = &[
        Step::Unlock,
        Step::MaskWrite(0xF8000110, 0x003FFFF0, 0x000FA3C0), // ARM_PLL_CFG
        Step::MaskWrite(0xF8000100, 0x0007F000, 0x0002E000), // ARM_PLL_CTRL
        Step::MaskWrite(0xF8000100, 0x00000010, 0x00000010), // ARM_PLL_CTRL
        Step::MaskWrite(0xF8000100, 0x00000001, 0x00000001), // ARM_PLL_CTRL
        Step::MaskWrite(0xF8000100, 0x00000001, 0x00000000), // ARM_PLL_CTRL
        Step::Poll(0xF800010C, 0x00000001),
        Step::MaskWrite(0xF8000100, 0x00000010, 0x00000000), // ARM_PLL_CTRL
        Step::MaskWrite(0xF8000120, 0x1F003F30, 0x1F000200), // ARM_CLK_CTRL
        Step::MaskWrite(0xF8000114, 0x003FFFF0, 0x0012C220), // DDR_PLL_CFG
        Step::MaskWrite(0xF8000104, 0x0007F000, 0x00020000), // DDR_PLL_CTRL
        Step::MaskWrite(0xF8000104, 0x00000010, 0x00000010), // DDR_PLL_CTRL
        Step::MaskWrite(0xF8000104, 0x00000001, 0x00000001), // DDR_PLL_CTRL
        Step::MaskWrite(0xF8000104, 0x00000001, 0x00000000), // DDR_PLL_CTRL
        Step::Poll(0xF800010C, 0x00000002),
        Step::MaskWrite(0xF8000104, 0x00000010, 0x00000000), // DDR_PLL_CTRL
        Step::MaskWrite(0xF8000124, 0xFFF00003, 0x0C200003), // DDR_CLK_CTRL
        Step::MaskWrite(0xF8000118, 0x003FFFF0, 0x001452C0), // IO_PLL_CFG
        Step::MaskWrite(0xF8000108, 0x0007F000, 0x0001E000), // IO_PLL_CTRL
        Step::MaskWrite(0xF8000108, 0x00000010, 0x00000010), // IO_PLL_CTRL
        Step::MaskWrite(0xF8000108, 0x00000001, 0x00000001), // IO_PLL_CTRL
        Step::MaskWrite(0xF8000108, 0x00000001, 0x00000000), // IO_PLL_CTRL
        Step::Poll(0xF800010C, 0x00000004),
        Step::MaskWrite(0xF8000108, 0x00000010, 0x00000000), // IO_PLL_CTRL
        Step::Lock,
    ];
    run(STEPS)
}

/// UART/SD/GEM and APER clocks (UG585 clock control).
unsafe fn clocks() -> Result<(), u32> {
    const STEPS: &[Step] = &[
        Step::Unlock,
        Step::MaskWrite(0xF8000128, 0x03F03F01, 0x00700F01),
        Step::MaskWrite(0xF8000138, 0x00000011, 0x00000001),
        Step::MaskWrite(0xF8000140, 0x03F03F71, 0x00100801), // GEM0_CLK_CTRL
        Step::MaskWrite(0xF800014C, 0x00003F31, 0x00000501),
        Step::MaskWrite(0xF8000150, 0x00003F33, 0x00000A01), // SDIO_CLK_CTRL
        Step::MaskWrite(0xF8000154, 0x00003F33, 0x00000A01),
        Step::MaskWrite(0xF8000168, 0x00003F31, 0x00000501), // UART_CLK_CTRL
        Step::MaskWrite(0xF8000170, 0x03F03F30, 0x00200500),
        Step::MaskWrite(0xF80001C4, 0x00000001, 0x00000001),
        Step::MaskWrite(0xF800012C, 0x01FFCCCD, 0x01DC044D),
        Step::Lock,
    ];
    run(STEPS)
}

/// DDRC + PHY for MT41K256M16-125, 16-bit, ~533 MHz.
unsafe fn ddr() -> Result<(), u32> {
    const STEPS: &[Step] = &[
        Step::MaskWrite(0xF8006000, 0x0001FFFF, 0x00000084), // ddrc_ctrl
        Step::MaskWrite(0xF8006004, 0x0007FFFF, 0x00001082), // Two_rank_cfg
        Step::MaskWrite(0xF8006008, 0x03FFFFFF, 0x03C0780F), // HPR_reg
        Step::MaskWrite(0xF800600C, 0x03FFFFFF, 0x02001001), // LPR_reg
        Step::MaskWrite(0xF8006010, 0x03FFFFFF, 0x00014001), // WR_reg
        Step::MaskWrite(0xF8006014, 0x001FFFFF, 0x0004285B), // DRAM_param_reg0
        Step::MaskWrite(0xF8006018, 0xF7FFFFFF, 0x44E458D3), // DRAM_param_reg1
        Step::MaskWrite(0xF800601C, 0xFFFFFFFF, 0x7282BCE5), // DRAM_param_reg2
        Step::MaskWrite(0xF8006020, 0x7FDFFFFC, 0x270872D0), // DRAM_param_reg3
        Step::MaskWrite(0xF8006024, 0x0FFFFFC3, 0x00000000), // DRAM_param_reg4
        Step::MaskWrite(0xF8006028, 0x00003FFF, 0x00002007), // DRAM_init_param
        Step::MaskWrite(0xF800602C, 0xFFFFFFFF, 0x00000008), // DRAM_EMR_reg
        Step::MaskWrite(0xF8006030, 0xFFFFFFFF, 0x00040B30), // DRAM_EMR_MR_reg
        Step::MaskWrite(0xF8006034, 0x13FF3FFF, 0x000116D4), // DRAM_burst8_rdwr
        Step::MaskWrite(0xF8006038, 0x00000003, 0x00000000), // DRAM_disable_DQ
        Step::MaskWrite(0xF800603C, 0x000FFFFF, 0x00000666), // DRAM_addr_map_bank
        Step::MaskWrite(0xF8006040, 0xFFFFFFFF, 0xFFFF0000), // DRAM_addr_map_col
        Step::MaskWrite(0xF8006044, 0x0FFFFFFF, 0x0F555555), // DRAM_addr_map_row
        Step::MaskWrite(0xF8006048, 0x0003F03F, 0x0003C008), // DRAM_ODT_reg
        Step::MaskWrite(0xF8006050, 0xFF0F8FFF, 0x77010800), // PHY_cmd_timeout
        Step::MaskWrite(0xF8006058, 0x00010000, 0x00000000),
        Step::MaskWrite(0xF800605C, 0x0000FFFF, 0x00005003),
        Step::MaskWrite(0xF8006060, 0x000017FF, 0x0000003E),
        Step::MaskWrite(0xF8006064, 0x00021FE0, 0x00020000),
        Step::MaskWrite(0xF8006068, 0x03FFFFFF, 0x00284141),
        Step::MaskWrite(0xF800606C, 0x0000FFFF, 0x00001610),
        Step::MaskWrite(0xF8006078, 0x03FFFFFF, 0x00466111),
        Step::MaskWrite(0xF800607C, 0x000FFFFF, 0x00032222),
        Step::MaskWrite(0xF80060A4, 0xFFFFFFFF, 0x10200802),
        Step::MaskWrite(0xF80060A8, 0x0FFFFFFF, 0x0690CB73),
        Step::MaskWrite(0xF80060AC, 0x000001FF, 0x000001FE),
        Step::MaskWrite(0xF80060B0, 0x1FFFFFFF, 0x1CFFFFFF),
        Step::MaskWrite(0xF80060B4, 0x00000200, 0x00000200),
        Step::MaskWrite(0xF80060B8, 0x01FFFFFF, 0x00200066),
        Step::MaskWrite(0xF80060C4, 0x00000003, 0x00000000),
        Step::MaskWrite(0xF80060C8, 0x000000FF, 0x00000000),
        Step::MaskWrite(0xF80060DC, 0x00000001, 0x00000000),
        Step::MaskWrite(0xF80060F0, 0x0000FFFF, 0x00000000),
        Step::MaskWrite(0xF80060F4, 0x0000000F, 0x00000008),
        Step::MaskWrite(0xF8006114, 0x000000FF, 0x00000000),
        Step::MaskWrite(0xF8006118, 0x7FFFFFCF, 0x40000001),
        Step::MaskWrite(0xF800611C, 0x7FFFFFCF, 0x40000001),
        Step::MaskWrite(0xF8006120, 0x7FFFFFCF, 0x40000000),
        Step::MaskWrite(0xF8006124, 0x7FFFFFCF, 0x40000000),
        Step::MaskWrite(0xF800612C, 0x000FFFFF, 0x00029000),
        Step::MaskWrite(0xF8006130, 0x000FFFFF, 0x00029000),
        Step::MaskWrite(0xF8006134, 0x000FFFFF, 0x00029000),
        Step::MaskWrite(0xF8006138, 0x000FFFFF, 0x00029000),
        Step::MaskWrite(0xF8006140, 0x000FFFFF, 0x00000035),
        Step::MaskWrite(0xF8006144, 0x000FFFFF, 0x00000035),
        Step::MaskWrite(0xF8006148, 0x000FFFFF, 0x00000035),
        Step::MaskWrite(0xF800614C, 0x000FFFFF, 0x00000035),
        Step::MaskWrite(0xF8006154, 0x000FFFFF, 0x00000080),
        Step::MaskWrite(0xF8006158, 0x000FFFFF, 0x00000080),
        Step::MaskWrite(0xF800615C, 0x000FFFFF, 0x00000080),
        Step::MaskWrite(0xF8006160, 0x000FFFFF, 0x00000080),
        Step::MaskWrite(0xF8006168, 0x001FFFFF, 0x000000F9),
        Step::MaskWrite(0xF800616C, 0x001FFFFF, 0x000000F9),
        Step::MaskWrite(0xF8006170, 0x001FFFFF, 0x000000F9),
        Step::MaskWrite(0xF8006174, 0x001FFFFF, 0x000000F9),
        Step::MaskWrite(0xF800617C, 0x000FFFFF, 0x000000C0),
        Step::MaskWrite(0xF8006180, 0x000FFFFF, 0x000000C0),
        Step::MaskWrite(0xF8006184, 0x000FFFFF, 0x000000C0),
        Step::MaskWrite(0xF8006188, 0x000FFFFF, 0x000000C0),
        Step::MaskWrite(0xF8006190, 0x6FFFFEFE, 0x00040080),
        Step::MaskWrite(0xF8006194, 0x000FFFFF, 0x0001FC82),
        Step::MaskWrite(0xF8006204, 0xFFFFFFFF, 0x00000000),
        Step::MaskWrite(0xF8006208, 0x000703FF, 0x000003FF),
        Step::MaskWrite(0xF800620C, 0x000703FF, 0x000003FF),
        Step::MaskWrite(0xF8006210, 0x000703FF, 0x000003FF),
        Step::MaskWrite(0xF8006214, 0x000703FF, 0x000003FF),
        Step::MaskWrite(0xF8006218, 0x000F03FF, 0x000003FF),
        Step::MaskWrite(0xF800621C, 0x000F03FF, 0x000003FF),
        Step::MaskWrite(0xF8006220, 0x000F03FF, 0x000003FF),
        Step::MaskWrite(0xF8006224, 0x000F03FF, 0x000003FF),
        Step::MaskWrite(0xF80062A8, 0x00000FF5, 0x00000000),
        Step::MaskWrite(0xF80062AC, 0xFFFFFFFF, 0x00000000),
        Step::MaskWrite(0xF80062B0, 0x003FFFFF, 0x00005125),
        Step::MaskWrite(0xF80062B4, 0x0003FFFF, 0x000012A8),
        Step::Poll(0xF8000B74, 0x00002000),
        Step::MaskWrite(0xF8006000, 0x0001FFFF, 0x00000085), // ddrc_ctrl
        Step::Poll(0xF8006054, 0x00000007),
    ];
    run(STEPS)
}

/// UART0, GPIO bank, DevCfg bits used before PCAP.
unsafe fn peripherals() -> Result<(), u32> {
    const STEPS: &[Step] = &[
        Step::Unlock,
        Step::MaskWrite(0xF8000B48, 0x00000180, 0x00000180),
        Step::MaskWrite(0xF8000B4C, 0x00000180, 0x00000000),
        Step::MaskWrite(0xF8000B50, 0x00000180, 0x00000180),
        Step::MaskWrite(0xF8000B54, 0x00000180, 0x00000000),
        Step::Lock,
        Step::MaskWrite(0xE0000034, 0x000000FF, 0x00000006),
        Step::MaskWrite(0xE0000018, 0x0000FFFF, 0x0000007C),
        Step::MaskWrite(0xE0000000, 0x000001FF, 0x00000017), // UART0 CR
        Step::MaskWrite(0xE0000004, 0x000003FF, 0x00000020),
        Step::MaskWrite(0xE000D000, 0x00080000, 0x00080000),
        Step::MaskWrite(0xF8007000, 0x20000000, 0x00000000),
        Step::MaskWrite(0xE000A244, 0x003FFFFF, 0x00004000),
        Step::MaskWrite(0xE000A008, 0xFFFFFFFF, 0xBFFF4000),
        Step::MaskWrite(0xE000A248, 0x003FFFFF, 0x00004000),
        Step::MaskWrite(0xE000A008, 0xFFFFFFFF, 0xBFFF0000),
        Step::DelayMs(1),
        Step::MaskWrite(0xE000A008, 0xFFFFFFFF, 0xBFFF4000),
    ];
    run(STEPS)
}

/// Full PS7 bring-up.
pub unsafe fn bringup() -> Result<(), u32> {
    mio()?;
    pll()?;
    clocks()?;
    // #region agent log
    crate::board::zynq7000::uart::write_str_bounded("STAGE1: PS7 clocks\n");
    // #endregion
    ddr()?;
    // #region agent log
    crate::board::zynq7000::uart::write_str_bounded("STAGE1: PS7 ddr\n");
    // #endregion
    peripherals()?;
    Ok(())
}

pub unsafe fn ddr_smoke(addr: usize) -> bool {
    let p = addr as *mut u32;
    let pattern = 0xA5A5_5A5Au32;
    core::ptr::write_volatile(p, pattern);
    core::ptr::write_volatile(p.add(1), !pattern);
    core::ptr::read_volatile(p) == pattern && core::ptr::read_volatile(p.add(1)) == !pattern
}
