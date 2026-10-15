use kairos::boot::arch::riscv64::trap::{handle_ecall, syscall_trap_count, TrapFrame};
use kairos::boot::memory_map::{
    CLINT_BASE, MTIME, MTIMECMP, UART_BASE, VIRTIO_MMIO_BASE, VIRTIO_MMIO_SLOTS, VIRTIO_MMIO_STRIDE,
};

#[test]
fn memory_map_matches_qemu_virt() {
    assert_eq!(UART_BASE, 0x1000_0000);
    assert_eq!(VIRTIO_MMIO_BASE, 0x1000_1000);
    assert_eq!(VIRTIO_MMIO_STRIDE, 0x1000);
    assert_eq!(VIRTIO_MMIO_SLOTS, 8);
    assert_eq!(CLINT_BASE, 0x0200_0000);
    assert_eq!(MTIME, CLINT_BASE + 0xBFF8);
    assert_eq!(MTIMECMP, CLINT_BASE + 0x4000);
}

#[test]
fn trap_ecall_enters_kernel_counter() {
    let before = syscall_trap_count();
    let mut frame = TrapFrame {
        sepc: 0x8000_0000,
        a0: 0,
        a7: kairos::kernel::syscall::WRITE as usize,
    };
    handle_ecall(&mut frame);
    assert_eq!(syscall_trap_count(), before + 1);
}

#[test]
fn boot_runtime_spawns_init() {
    let mut rt = kairos::Runtime::new();
    let pid = rt.boot().unwrap();
    assert!(rt.hosted.kernel.get_process(pid).is_ok());
}

#[test]
fn boot_splash_text_matches_qemu_smoke() {
    use kairos::userland::splash::{self, BootSplashInfo};
    assert_eq!(
        splash::BOOT_SPLASH,
        "Golden Phase Operating System boot splash"
    );
    let mut kernel = kairos::Kernel::new();
    let pid = kernel.spawn_process("init");
    let info = BootSplashInfo::from_kernel(&kernel, pid, false);
    let text = splash::render_boot_screen(&info);
    assert!(text.contains("Golden Phase Operating System boot splash"));
    assert!(text.contains("awakening"));
    assert!(text.contains("stateclock"));
    assert!(text.contains("scheduler"));
}

#[test]
fn timer_preempt_on_boot_runtime() {
    let mut rt = kairos::Runtime::new();
    rt.boot().unwrap();
    let child = rt.spawn_background().unwrap();
    rt.hosted.kernel.schedule(child).unwrap();
    rt.force_timer_tick().unwrap();
    assert!(rt.trace.events.len() >= 2);
}
