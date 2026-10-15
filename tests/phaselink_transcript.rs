use kairos::Runtime;

#[test]
fn hosted_phaselink_transcript() {
    let mut rt = Runtime::with_input([
        "phaselink encode A",
        "phaselink decode Φ1 Φ0 Φ0 Φ1",
        "phaselink loopback HELLO",
        "phaselink sim init",
        "phaselink sim send node-a node-b SENSOR_OK",
        "phaselink sim tamper payload",
        "phaselink sim send node-a node-b SENSOR_BAD",
        "phaselink sim replay",
        "phaselink sim reboot node-b",
        "phaselink status",
        "phaselink chain",
        "phaselink root",
        "phaselink quarantine",
        "phaselink gateway status",
        "exit",
    ]);
    rt.boot().unwrap();
    let pid = rt.hosted.init_pid.unwrap();
    rt.hosted.kernel.syscall(pid).unwrap();
    rt.hosted.kernel.kreturn(pid).unwrap();
    rt.hosted.kernel.schedule(pid).unwrap();
    rt.run().unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("phase stream"));
    assert!(out.contains("[ RESULT ] HELLO"));
    assert!(out.contains("node-a: ready"));
    assert!(out.contains("receive result: ACCEPTED"));
    assert!(out.contains("tamper armed: payload"));
    assert!(out.contains("replay queued"));
    assert!(out.contains("node-b: rebooted"));
    assert!(out.contains("PhaseLinkRoot"));
    assert!(out.contains("gateway: disabled"));
}
