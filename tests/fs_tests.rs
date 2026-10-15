use kairos::core::gphase_core::{compose, PhaseState};
use kairos::fs::{FsError, PhaseFs};
use kairos::userland::fs_cmd;
use kairos::userland::init::{boot, start_shell};
use kairos::userland::Hosted;

#[test]
fn create_write_read_delete_file() {
    let mut fs = PhaseFs::new();
    fs.create_file("/hello.txt").unwrap();
    fs.write_file("/hello.txt", b"phase").unwrap();
    assert_eq!(fs.read_file("/hello.txt").unwrap(), b"phase");
    fs.write_file("/hello.txt", b"updated").unwrap();
    assert_eq!(fs.read_file("/hello.txt").unwrap(), b"updated");
    fs.delete_file("/hello.txt").unwrap();
    assert!(matches!(
        fs.read_file("/hello.txt"),
        Err(FsError::NotFound(_))
    ));
}

#[test]
fn mkdir_list_nested_paths() {
    let mut fs = PhaseFs::new();
    fs.mkdir("/docs").unwrap();
    fs.create_file("/docs/readme.txt").unwrap();
    fs.write_file("/docs/readme.txt", b"nested").unwrap();
    let listing = fs.list_dir("/docs").unwrap();
    assert!(listing.contains(&"readme.txt".to_string()));
    assert_eq!(fs.read_file("/docs/readme.txt").unwrap(), b"nested");
}

#[test]
fn refuse_duplicate_names_and_missing_reads() {
    let mut fs = PhaseFs::new();
    fs.create_file("/dup.txt").unwrap();
    assert!(matches!(
        fs.create_file("/dup.txt"),
        Err(FsError::AlreadyExists(_))
    ));
    assert!(matches!(
        fs.read_file("/missing.txt"),
        Err(FsError::NotFound(_))
    ));
}

#[test]
fn block_tags_follow_phase_model() {
    let mut fs = PhaseFs::new();
    fs.mkdir("/docs").unwrap();
    fs.create_file("/docs/a.txt").unwrap();
    fs.write_file("/docs/a.txt", b"x").unwrap();
    assert_eq!(fs.stat("/docs").unwrap(), PhaseState::Phi2);
    assert_eq!(fs.stat("/docs/a.txt").unwrap(), PhaseState::Phi1);
    assert_eq!(fs.block_tag("/docs/a.txt").unwrap(), PhaseState::Phi1);
    fs.lock_path("/docs/a.txt").unwrap();
    assert_eq!(fs.stat("/docs/a.txt").unwrap(), PhaseState::Phi3);
    assert_eq!(fs.block_tag("/docs/a.txt").unwrap(), PhaseState::Phi3);
    assert!(matches!(
        fs.delete_file("/docs/a.txt"),
        Err(FsError::Locked(_))
    ));
    fs.unlock_path("/docs/a.txt").unwrap();
    assert_eq!(fs.stat("/docs/a.txt").unwrap(), PhaseState::Phi1);
    fs.delete_file("/docs/a.txt").unwrap();
    assert!(matches!(
        fs.block_tag("/docs/a.txt"),
        Err(FsError::NotFound(_))
    ));
}

#[test]
fn locked_block_cannot_be_freed_directly() {
    let mut fs = PhaseFs::new();
    fs.create_file("/locked.txt").unwrap();
    fs.lock_path("/locked.txt").unwrap();
    let block = fs.block_tag("/locked.txt").unwrap();
    assert_eq!(block, PhaseState::Phi3);
}

#[test]
fn contention_uses_compose_without_auto_mutation() {
    let mut fs = PhaseFs::new();
    fs.create_file("/a.txt").unwrap();
    fs.create_file("/b.txt").unwrap();
    assert_eq!(
        fs.contention_check("/a.txt", "/b.txt").unwrap(),
        compose(PhaseState::Phi1, PhaseState::Phi1)
    );
    assert_eq!(fs.stat("/a.txt").unwrap(), PhaseState::Phi1);
    fs.mkdir("/d1").unwrap();
    fs.mkdir("/d2").unwrap();
    assert_eq!(
        fs.contention_check("/d1", "/d2").unwrap(),
        compose(PhaseState::Phi2, PhaseState::Phi2)
    );
    assert_eq!(
        fs.contention_check("/a.txt", "/a.txt").unwrap(),
        PhaseState::Phi3
    );
    assert_eq!(fs.stat("/a.txt").unwrap(), PhaseState::Phi1);
}

#[test]
fn journal_recovery_replays_pending_write() {
    let mut fs = PhaseFs::new();
    fs.mkdir("/docs").unwrap();
    fs.create_file("/docs/a.txt").unwrap();
    fs.set_pending_journal_for_test("/docs/a.txt", b"saved");
    fs.recover_journal().unwrap();
    assert_eq!(fs.read_file("/docs/a.txt").unwrap(), b"saved");
}

#[test]
fn shell_filesystem_integration() {
    let mut hosted = Hosted::with_input([
        "mkdir /docs",
        "write /docs/readme.txt hello",
        "cat /docs/readme.txt",
        "stat /docs/readme.txt",
        "lock /docs/readme.txt",
        "stat /docs/readme.txt",
        "rm /docs/readme.txt",
        "unlock /docs/readme.txt",
        "rm /docs/readme.txt",
        "ls /docs",
        "exit",
    ]);
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();
    start_shell(&mut hosted).unwrap();

    let output = hosted.output_lines().join("\n");
    assert!(output.contains("hello"));
    assert!(output.contains("Kairos1"));
    assert!(output.contains("Kairos3"));
    assert!(output.contains("error: cannot remove locked path"));
    assert!(!output.contains("readme.txt") || output.contains("ls /docs"));
}

#[test]
fn shell_ls_mkdir_rm() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.execute_line(pid, "mkdir /tmp").unwrap();
    hosted.execute_line(pid, "ls /").unwrap();
    let text = hosted.output_lines().last().unwrap();
    assert!(text.contains("tmp"));
}

#[test]
fn phase_label_helper() {
    assert_eq!(fs_cmd::phase_label(PhaseState::Phi1), "Kairos1");
}

#[test]
fn new_inodes_start_with_zeroed_phase_time() {
    let mut fs = PhaseFs::new();
    fs.create_file("/fresh.txt").unwrap();
    let id = fs.inode_id_for_path("/fresh.txt").unwrap();
    let inode = fs.inode(id).unwrap();
    assert_eq!(inode.chronos, 0);
    assert_eq!(inode.kairos_sector, 0);
}

#[test]
fn stamp_phase_time_updates_the_resolved_inode() {
    let mut fs = PhaseFs::new();
    fs.create_file("/stamped.txt").unwrap();
    let id = fs.stamp_phase_time("/stamped.txt", 42, 3).unwrap();
    let inode = fs.inode(id).unwrap();
    assert_eq!(inode.chronos, 42);
    assert_eq!(inode.kairos_sector, 3);

    // Re-stamping with a new tick moves the inode forward, not additively.
    fs.stamp_phase_time("/stamped.txt", 100, 1).unwrap();
    let inode = fs.inode(id).unwrap();
    assert_eq!(inode.chronos, 100);
    assert_eq!(inode.kairos_sector, 1);
}

#[test]
fn stamp_phase_time_on_missing_path_errors() {
    let mut fs = PhaseFs::new();
    assert!(matches!(
        fs.stamp_phase_time("/nope.txt", 1, 0),
        Err(FsError::NotFound(_))
    ));
}

#[test]
fn iter_inodes_reflects_current_tree() {
    let mut fs = PhaseFs::new();
    fs.mkdir("/docs").unwrap();
    fs.create_file("/docs/a.txt").unwrap();
    fs.create_file("/docs/b.txt").unwrap();
    let names: Vec<String> = fs.iter_inodes().map(|i| i.name.clone()).collect();
    assert!(names.contains(&"a.txt".to_string()));
    assert!(names.contains(&"b.txt".to_string()));
    assert!(names.contains(&"docs".to_string()));
}
