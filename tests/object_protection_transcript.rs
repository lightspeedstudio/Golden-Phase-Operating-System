//! Hosted command transcript for per-object ANOM protection.

use kairos::Runtime;

const PASS: &str = "demo-object-pass";
const NEW_PASS: &str = "demo-object-new-pass";

fn run_shell(rt: &mut Runtime) {
    let pid = rt.hosted.init_pid.unwrap();
    rt.hosted.kernel.syscall(pid).unwrap();
    rt.hosted.kernel.kreturn(pid).unwrap();
    rt.hosted.kernel.schedule(pid).unwrap();
    rt.run().unwrap();
}

#[test]
fn hosted_object_protection_transcript() {
    let _ = std::fs::remove_dir_all(".kairos/anom/objects");
    let mut rt = Runtime::with_input([
        "anom protect process:0 -p",
        PASS,
        PASS,
        "ps",
        "anom status process:0",
        "anom open process:0 -p",
        PASS,
        "anom close process:0",
        "anom rekey process:0 -p",
        PASS,
        NEW_PASS,
        NEW_PASS,
        "anom open process:0 -p",
        "wrong-old-pass",
        "anom open process:0 -p",
        NEW_PASS,
        "exit",
    ]);
    rt.boot().unwrap();
    run_shell(&mut rt);
    let out = rt.output_lines().join("\n");
    println!("--- HOSTED TRANSCRIPT ---\n{out}\n--- END ---");
    assert!(out.contains("ANOM-protected") || out.contains("process:0"));
    assert!(out.contains("ANOM:"));
    assert!(out.contains("protected/locked"));
    assert!(out.contains("true state:"));
    assert!(out.contains("object rekeyed") || out.contains("rekey"));
    assert!(out.contains("object unlock failed") || out.contains("authentication failed"));
}
