//! Host crash-injection matrix for the PhaseFS journal.

use super::*;
use crate::fs::blockdev::{BlockDevice, RamBlockDev};
use crate::fs::persist;
use crate::fs::phasefs::PhaseFs;
use crate::fs::journal::wal::PhaseFsJournal;

fn recover_from(j: PhaseFsJournal<RamBlockDev>) -> (PhaseFs, RecoverReport, PhaseFsJournal<RamBlockDev>) {
    let dev = j.into_device();
    let mut j2 = PhaseFsJournal::attach(dev).expect("attach");
    let mut fs = PhaseFs::new();
    let report = j2.recover(&mut fs).expect("recover");
    check_invariants(&fs).expect("invariants");
    (fs, report, j2)
}

fn committed_new_state(setup: &[RedoOp], op: &RedoOp) -> (Vec<u8>, [u8; 32]) {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    if !setup.is_empty() {
        j.run_transaction(&mut fs, setup).unwrap();
    }
    j.run_transaction(&mut fs, core::slice::from_ref(op)).unwrap();
    (persist::encode(&fs).unwrap(), persist::content_root(&fs))
}

fn run_crash(setup: &[RedoOp], op: RedoOp, crash: CrashPoint) {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    if !setup.is_empty() {
        j.run_transaction(&mut fs, setup).unwrap();
    }
    let old_bytes = persist::encode(&fs).unwrap();
    let old_root = persist::content_root(&fs);
    let (new_bytes, new_root) = committed_new_state(setup, &op);

    j.set_crash(crash);
    let result = j.run_transaction(&mut fs, core::slice::from_ref(&op));
    if crash == CrashPoint::None {
        assert!(result.is_ok());
    } else {
        assert_eq!(result, Err(JournalError::InjectedCrash));
    }

    let (fs2, _report, j2) = recover_from(j);
    let got = persist::encode(&fs2).unwrap();
    let got_root = persist::content_root(&fs2);

    let before_commit = matches!(
        crash,
        CrashPoint::BeforeBegin
            | CrashPoint::AfterBegin
            | CrashPoint::AfterRedo
            | CrashPoint::AfterJournalFlush
    );
    if before_commit {
        assert_eq!(got_root, old_root, "crash {crash:?} must keep old state");
        assert_eq!(got, old_bytes);
    } else {
        assert_eq!(got_root, new_root, "crash {crash:?} must finish new state");
        assert_eq!(got, new_bytes);
    }

    // Recover twice.
    let (fs3, _, _) = recover_from(j2);
    assert_eq!(persist::content_root(&fs3), got_root);
    check_invariants(&fs3).unwrap();
}

fn all_crashes() -> [CrashPoint; 11] {
    [
        CrashPoint::BeforeBegin,
        CrashPoint::AfterBegin,
        CrashPoint::AfterRedo,
        CrashPoint::AfterJournalFlush,
        CrashPoint::AfterCommit,
        CrashPoint::AfterCommitFlush,
        CrashPoint::DuringApply,
        CrashPoint::AfterApply,
        CrashPoint::AfterApplyFlush,
        CrashPoint::AfterApplied,
        CrashPoint::BeforeTailAdvance,
    ]
}

#[test]
fn format_and_clean_commit() {
    let mut fs = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs).unwrap();
    j.run_transaction(
        &mut fs,
        &[RedoOp::Write {
            path: "/a.txt".into(),
            data: b"hello".to_vec(),
        }],
    )
    .unwrap();
    check_invariants(&fs).unwrap();
    let (mut fs2, _, _) = recover_from(j);
    assert_eq!(fs2.read_file("/a.txt").unwrap(), b"hello");
}

#[test]
fn create_crash_matrix() {
    let op = RedoOp::Create {
        path: "/n.txt".into(),
    };
    for c in all_crashes() {
        run_crash(&[], op.clone(), c);
    }
}

#[test]
fn write_crash_matrix() {
    let setup = [RedoOp::Create {
        path: "/w.txt".into(),
    }];
    let op = RedoOp::Write {
        path: "/w.txt".into(),
        data: b"xyz".to_vec(),
    };
    for c in all_crashes() {
        run_crash(&setup, op.clone(), c);
    }
}

#[test]
fn mkdir_crash_matrix() {
    let op = RedoOp::Mkdir {
        path: "/d".into(),
    };
    for c in all_crashes() {
        run_crash(&[], op.clone(), c);
    }
}

#[test]
fn rename_crash_matrix() {
    let setup = [RedoOp::Write {
        path: "/old.txt".into(),
        data: b"v".to_vec(),
    }];
    let op = RedoOp::Rename {
        from: "/old.txt".into(),
        to: "/new.txt".into(),
    };
    for c in all_crashes() {
        run_crash(&setup, op.clone(), c);
    }
}

#[test]
fn rename_commit_preserves_data_and_updates_owner_path() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    j.run_transaction(
        &mut fs,
        &[RedoOp::Write {
            path: "/old.txt".into(),
            data: b"payload".to_vec(),
        }],
    )
    .unwrap();
    j.run_transaction(
        &mut fs,
        &[RedoOp::Rename {
            from: "/old.txt".into(),
            to: "/new.txt".into(),
        }],
    )
    .unwrap();
    assert_eq!(fs.read_file("/new.txt").unwrap(), b"payload");
    assert!(fs.read_file("/old.txt").is_err());
    let id = fs.inode_id_for_path("/new.txt").unwrap();
    assert_eq!(fs.primary_block_owner(id).as_deref(), Some("/new.txt"));

    let (mut fs2, _, _) = recover_from(j);
    assert_eq!(fs2.read_file("/new.txt").unwrap(), b"payload");
    assert!(fs2.read_file("/old.txt").is_err());
    let id2 = fs2.inode_id_for_path("/new.txt").unwrap();
    assert_eq!(fs2.primary_block_owner(id2).as_deref(), Some("/new.txt"));
}

#[test]
fn rename_commit_moves_directory() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    j.run_transaction(
        &mut fs,
        &[
            RedoOp::Mkdir {
                path: "/docs".into(),
            },
            RedoOp::Write {
                path: "/docs/a.txt".into(),
                data: b"child".to_vec(),
            },
        ],
    )
    .unwrap();
    j.run_transaction(
        &mut fs,
        &[RedoOp::Rename {
            from: "/docs".into(),
            to: "/papers".into(),
        }],
    )
    .unwrap();
    assert!(fs.read_file("/docs/a.txt").is_err());
    assert_eq!(fs.read_file("/papers/a.txt").unwrap(), b"child");
}

#[test]
fn rename_transaction_refuses_missing_source() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    assert_eq!(
        j.run_transaction(
            &mut fs,
            &[RedoOp::Rename {
                from: "/missing.txt".into(),
                to: "/x.txt".into(),
            }],
        ),
        Err(JournalError::Fs)
    );
    assert!(fs.read_file("/x.txt").is_err());
}

#[test]
fn rename_transaction_refuses_collision() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    j.run_transaction(
        &mut fs,
        &[
            RedoOp::Write {
                path: "/a.txt".into(),
                data: b"a".to_vec(),
            },
            RedoOp::Write {
                path: "/b.txt".into(),
                data: b"b".to_vec(),
            },
        ],
    )
    .unwrap();
    assert_eq!(
        j.run_transaction(
            &mut fs,
            &[RedoOp::Rename {
                from: "/a.txt".into(),
                to: "/b.txt".into(),
            }],
        ),
        Err(JournalError::Fs)
    );
    assert_eq!(fs.read_file("/a.txt").unwrap(), b"a");
    assert_eq!(fs.read_file("/b.txt").unwrap(), b"b");
}

#[test]
fn remove_crash_matrix() {
    let setup = [RedoOp::Write {
        path: "/gone.txt".into(),
        data: b"z".to_vec(),
    }];
    let op = RedoOp::Remove {
        path: "/gone.txt".into(),
    };
    for c in all_crashes() {
        run_crash(&setup, op.clone(), c);
    }
}

#[test]
fn lock_unlock_crash_matrix() {
    let setup = [RedoOp::Write {
        path: "/l.txt".into(),
        data: b"k".to_vec(),
    }];
    let lock = RedoOp::Lock {
        path: "/l.txt".into(),
    };
    for c in all_crashes() {
        run_crash(&setup, lock.clone(), c);
    }
    let setup2 = [
        RedoOp::Write {
            path: "/l.txt".into(),
            data: b"k".to_vec(),
        },
        RedoOp::Lock {
            path: "/l.txt".into(),
        },
    ];
    let unlock = RedoOp::Unlock {
        path: "/l.txt".into(),
    };
    // setup2 is two ops — run_crash only does one setup transaction. Fold lock into first txn:
    let fs0 = PhaseFs::new();
    for c in all_crashes() {
        let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
        let mut fs = PhaseFs::new();
        j.run_transaction(&mut fs, &setup2).unwrap();
        run_crash_on(j, fs, unlock.clone(), c);
    }
}

fn run_crash_on(
    mut j: PhaseFsJournal<RamBlockDev>,
    mut fs: PhaseFs,
    op: RedoOp,
    crash: CrashPoint,
) {
    let old_bytes = persist::encode(&fs).unwrap();
    let old_root = persist::content_root(&fs);
    let mut fs_new = persist::decode(&old_bytes).unwrap();
    fs_new.apply_redo(&op).unwrap();
    let new_bytes = persist::encode(&fs_new).unwrap();
    let new_root = persist::content_root(&fs_new);

    j.set_crash(crash);
    let result = j.run_transaction(&mut fs, core::slice::from_ref(&op));
    if crash == CrashPoint::None {
        assert!(result.is_ok());
    } else {
        assert_eq!(result, Err(JournalError::InjectedCrash));
    }
    let (fs2, _, j2) = recover_from(j);
    let got_root = persist::content_root(&fs2);
    let before_commit = matches!(
        crash,
        CrashPoint::BeforeBegin
            | CrashPoint::AfterBegin
            | CrashPoint::AfterRedo
            | CrashPoint::AfterJournalFlush
    );
    if before_commit {
        assert_eq!(got_root, old_root);
        assert_eq!(persist::encode(&fs2).unwrap(), old_bytes);
    } else {
        assert_eq!(got_root, new_root);
        assert_eq!(persist::encode(&fs2).unwrap(), new_bytes);
    }
    let (fs3, _, _) = recover_from(j2);
    assert_eq!(persist::content_root(&fs3), got_root);
}

#[test]
fn stamp_crash_matrix() {
    let setup = [RedoOp::Write {
        path: "/s.txt".into(),
        data: b"s".to_vec(),
    }];
    let op = RedoOp::Stamp {
        path: "/s.txt".into(),
        chronos: 42,
        kairos_sector: 3,
    };
    for c in all_crashes() {
        run_crash(&setup, op.clone(), c);
    }
}

#[test]
fn torn_final_sector_discards_uncommitted() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    j.set_crash(CrashPoint::AfterBegin);
    let _ = j.run_transaction(
        &mut fs,
        &[RedoOp::Write {
            path: "/t.txt".into(),
            data: b"torn".to_vec(),
        }],
    );
    let mut dev = j.into_device();
    dev.corrupt(WAL_START, 200, 0xFF);
    let mut j2 = PhaseFsJournal::attach(dev).unwrap();
    let mut fs2 = PhaseFs::new();
    j2.recover(&mut fs2).unwrap();
    check_invariants(&fs2).unwrap();
    assert!(fs2.read_file("/t.txt").is_err());
}

#[test]
fn wraparound_then_recover() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    for i in 0..520u32 {
        let path = format!("/f{i}.txt");
        j.run_transaction(
            &mut fs,
            &[RedoOp::Write {
                path,
                data: b"n".to_vec(),
            }],
        )
        .unwrap();
        if i > 0 {
            let prev = format!("/f{}.txt", i - 1);
            j.run_transaction(&mut fs, &[RedoOp::Remove { path: prev }])
                .unwrap();
        }
    }
    check_invariants(&fs).unwrap();
    let (mut fs2, _, _) = recover_from(j);
    check_invariants(&fs2).unwrap();
    assert!(fs2.read_file("/f519.txt").is_ok());
}

#[test]
fn unformatted_attach_fails() {
    let dev = RamBlockDev::new(REGION_SECTORS);
    assert!(matches!(
        PhaseFsJournal::attach(dev),
        Err(JournalError::Unformatted)
    ));
}

#[test]
fn redo_idempotent_apply_twice() {
    let mut fs = PhaseFs::new();
    let op = RedoOp::Mkdir {
        path: "/x".into(),
    };
    fs.apply_redo(&op).unwrap();
    fs.apply_redo(&op).unwrap();
    fs.apply_redo(&RedoOp::Create {
        path: "/x/a.txt".into(),
    })
    .unwrap();
    fs.apply_redo(&RedoOp::Write {
        path: "/x/a.txt".into(),
        data: b"q".to_vec(),
    })
    .unwrap();
    fs.apply_redo(&RedoOp::Write {
        path: "/x/a.txt".into(),
        data: b"q".to_vec(),
    })
    .unwrap();
    fs.apply_redo(&RedoOp::Rename {
        from: "/x/a.txt".into(),
        to: "/x/b.txt".into(),
    })
    .unwrap();
    fs.apply_redo(&RedoOp::Rename {
        from: "/x/a.txt".into(),
        to: "/x/b.txt".into(),
    })
    .unwrap();
    fs.apply_redo(&RedoOp::Lock {
        path: "/x/b.txt".into(),
    })
    .unwrap();
    fs.apply_redo(&RedoOp::Lock {
        path: "/x/b.txt".into(),
    })
    .unwrap();
    fs.apply_redo(&RedoOp::Unlock {
        path: "/x/b.txt".into(),
    })
    .unwrap();
    fs.apply_redo(&RedoOp::Unlock {
        path: "/x/b.txt".into(),
    })
    .unwrap();
    fs.apply_redo(&RedoOp::Remove {
        path: "/x/b.txt".into(),
    })
    .unwrap();
    fs.apply_redo(&RedoOp::Remove {
        path: "/x/b.txt".into(),
    })
    .unwrap();
    check_invariants(&fs).unwrap();
}

#[test]
fn corrupt_superblock_unformatted() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    j.run_transaction(
        &mut fs,
        &[RedoOp::Write {
            path: "/a.txt".into(),
            data: b"x".to_vec(),
        }],
    )
    .unwrap();
    let mut dev = j.into_device();
    dev.corrupt(SUPERBLOCK_A, 10, 0xFF);
    dev.corrupt(SUPERBLOCK_B, 10, 0xFF);
    assert!(matches!(
        PhaseFsJournal::attach(dev),
        Err(JournalError::Unformatted) | Err(JournalError::Checksum) | Err(JournalError::BadVersion)
    ));
}

#[test]
fn corrupt_record_checksum_rejected() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    j.set_crash(CrashPoint::AfterBegin);
    let _ = j.run_transaction(
        &mut fs,
        &[RedoOp::Write {
            path: "/c.txt".into(),
            data: b"no".to_vec(),
        }],
    );
    let mut dev = j.into_device();
    dev.corrupt(WAL_START, 8, 0xFF);
    let mut j2 = PhaseFsJournal::attach(dev).unwrap();
    let mut fs2 = PhaseFs::new();
    let report = j2.recover(&mut fs2).unwrap();
    assert_eq!(report.class, RecoverClass::RejectedCorrupt);
    assert!(!report.replayed);
    assert!(fs2.read_file("/c.txt").is_err());
    check_invariants(&fs2).unwrap();
    let (fs3, report2, _) = recover_from(j2);
    assert_eq!(report2.class, RecoverClass::RejectedCorrupt);
    assert_eq!(persist::content_root(&fs3), persist::content_root(&fs2));
}

#[test]
fn format_clears_rejected_corrupt() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    j.run_transaction(
        &mut fs,
        &[RedoOp::Write {
            path: "/keep.txt".into(),
            data: b"keep".to_vec(),
        }],
    )
    .unwrap();
    let (head_before, _, _) = j.wal_cursor();
    j.set_crash(CrashPoint::AfterBegin);
    let _ = j.run_transaction(
        &mut fs,
        &[RedoOp::Write {
            path: "/torn.txt".into(),
            data: b"no".to_vec(),
        }],
    );
    let mut dev = j.into_device();
    dev.corrupt(WAL_START + head_before, 8, 0xFF);
    let mut j2 = PhaseFsJournal::attach(dev).unwrap();
    let mut fs2 = PhaseFs::new();
    let report = j2.recover(&mut fs2).unwrap();
    assert_eq!(report.class, RecoverClass::RejectedCorrupt);
    assert!(report.discarded);
    assert_eq!(fs2.read_file("/keep.txt").unwrap(), b"keep");
    assert!(fs2.read_file("/torn.txt").is_err());

    let j3 = PhaseFsJournal::format(j2.into_device(), &fs2).unwrap();
    let (mut fs3, report2, _) = recover_from(j3);
    assert_eq!(report2.class, RecoverClass::Clean);
    assert!(!report2.discarded);
    assert!(!report2.replayed);
    assert_eq!(fs3.read_file("/keep.txt").unwrap(), b"keep");
    assert!(fs3.read_file("/torn.txt").is_err());
}

#[test]
fn missing_commit_isolated() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    j.set_crash(CrashPoint::AfterRedo);
    let _ = j.run_transaction(
        &mut fs,
        &[RedoOp::Write {
            path: "/m.txt".into(),
            data: b"no".to_vec(),
        }],
    );
    let (mut fs2, report, _) = recover_from(j);
    assert_eq!(report.class, RecoverClass::Isolated);
    assert!(fs2.read_file("/m.txt").is_err());
}

#[test]
fn old_generation_rejected() {
    use super::format::{encode_record, RawRecord};
    let fs0 = PhaseFs::new();
    let j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut dev = j.into_device();
    let rec = RawRecord {
        op_type: OP_BEGIN,
        txn_id: 0,
        record_seq: 0,
        target: 0,
        payload: Vec::new(),
        prev_root: [0u8; 32],
    };
    let bytes = encode_record(&rec).unwrap();
    let mut sec = [0u8; 512];
    sec.copy_from_slice(&bytes[..512]);
    dev.write(WAL_START, &sec).unwrap();
    let mut j2 = PhaseFsJournal::attach(dev).unwrap();
    let mut fs2 = PhaseFs::new();
    let report = j2.recover(&mut fs2).unwrap();
    assert_eq!(report.class, RecoverClass::RejectedCorrupt);
    assert!(!report.replayed);
}

#[test]
fn random_media_unformatted() {
    let dev = RamBlockDev::new(REGION_SECTORS);
    assert_eq!(
        PhaseFsJournal::attach(dev).err(),
        Some(JournalError::Unformatted)
    );
}

#[test]
fn applied_recover_keeps_checkpoint_generation() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    j.set_crash(CrashPoint::BeforeTailAdvance);
    let _ = j.run_transaction(
        &mut fs,
        &[RedoOp::Write {
            path: "/g.txt".into(),
            data: b"gen".to_vec(),
        }],
    );
    let (mut fs2, report, mut j2) = recover_from(j);
    assert_eq!(report.class, RecoverClass::Clean);
    assert_eq!(j2.checkpoint_generation(), 2);
    assert_eq!(fs2.read_file("/g.txt").unwrap(), b"gen");
    j2.set_crash(CrashPoint::None);
    j2.run_transaction(
        &mut fs2,
        &[RedoOp::Write {
            path: "/h.txt".into(),
            data: b"next".to_vec(),
        }],
    )
    .unwrap();
    assert_eq!(j2.checkpoint_generation(), 3);
    let (mut fs3, _, j3) = recover_from(j2);
    assert_eq!(j3.checkpoint_generation(), 3);
    assert_eq!(fs3.read_file("/h.txt").unwrap(), b"next");
}

#[test]
fn committed_recover_class() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    j.set_crash(CrashPoint::AfterCommitFlush);
    let _ = j.run_transaction(
        &mut fs,
        &[RedoOp::Write {
            path: "/r.txt".into(),
            data: b"yes".to_vec(),
        }],
    );
    let (mut fs2, report, _) = recover_from(j);
    assert_eq!(report.class, RecoverClass::Recovered);
    assert!(report.replayed);
    assert_eq!(fs2.read_file("/r.txt").unwrap(), b"yes");
}

const P6_GOOD: &str = "\
P6_ARMED run=11 gen=14 txn=11
P6_TRANSACTION_COMMITTED run=11
fsj CRASH point=5 — hardware reset
P6_RECOVERY_BEGIN run=11 gen=14
P6_RECOVERY_CRASH_FIRED run=11 stage=BeforeReplayApply
fsj CRASH point=100 — hardware reset
fsj: recovered class=RECOVERED replayed=1 discarded=0 gen=15 txn=11
p6-20260815_122229
fsj check: PASS
fsj recover: class=CLEAN replayed=0 discarded=0 gen=15
";

#[test]
fn p6_transcript_115741_is_not_pass() {
    let text = include_str!(
        "../../../fpga/zynq/research/phasefs-persistent-journal-v1/com12_fsjournal_p6_20260815_115741.txt"
    );
    let v = classify_p6_evidence(text, Some("p6r-20260815_115741"));
    assert_eq!(v, crate::fs::journal::P6Verdict::OpenNoResetDuringRecovery);
    assert!(!text.contains("P6_RECOVERY_CRASH_FIRED"));
}

#[test]
fn p6_transcript_122229_is_pass() {
    let text = include_str!(
        "../../../fpga/zynq/research/phasefs-persistent-journal-v1/com12_fsjournal_p6_20260815_122229.txt"
    );
    let v = classify_p6_evidence(text, Some("p6-20260815_122229"));
    assert_eq!(v, crate::fs::journal::P6Verdict::Pass);
}

#[test]
fn p6_classifier_negatives() {
    use crate::fs::journal::P6Verdict::*;
    assert_eq!(classify_p6_evidence(P6_GOOD, Some("p6-20260815_122229")), Pass);
    let one = P6_GOOD.replace("fsj CRASH point=100 — hardware reset\n", "");
    assert_eq!(classify_p6_evidence(&one, Some("p6-20260815_122229")), OpenNoResetDuringRecovery);
    let no_fire = P6_GOOD.replace(
        "P6_RECOVERY_CRASH_FIRED run=11 stage=BeforeReplayApply\n",
        "",
    );
    assert_eq!(classify_p6_evidence(&no_fire, Some("p6-20260815_122229")), OpenNoResetDuringRecovery);
    let no_begin = P6_GOOD.replace("P6_RECOVERY_BEGIN run=11 gen=14\n", "");
    assert_eq!(classify_p6_evidence(&no_begin, Some("p6-20260815_122229")), MismatchedRun);
    let bad_run = P6_GOOD.replace(
        "P6_RECOVERY_CRASH_FIRED run=11",
        "P6_RECOVERY_CRASH_FIRED run=99",
    );
    assert_eq!(classify_p6_evidence(&bad_run, Some("p6-20260815_122229")), MismatchedRun);
    assert_eq!(classify_p6_evidence(P6_GOOD, Some("wrong-bytes")), MissingFile);
    let no_clean = P6_GOOD.replace(
        "fsj recover: class=CLEAN replayed=0 discarded=0 gen=15\n",
        "",
    );
    assert_eq!(classify_p6_evidence(&no_clean, Some("p6-20260815_122229")), MissingClean);
    let io = format!("{P6_GOOD}journal i/o\n");
    assert_eq!(classify_p6_evidence(&io, Some("p6-20260815_122229")), FailIo);
    let reused = format!("P6_ARMED run=1\nP6_TRANSACTION_COMMITTED run=1\n{P6_GOOD}");
    assert_eq!(classify_p6_evidence(&reused, Some("p6-20260815_122229")), MismatchedRun);
}

#[test]
fn p6_plan_survives_reboot_and_fires_once() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    let run = j.next_txn_id();
    j.arm_p6(run).unwrap();
    j.set_crash(CrashPoint::AfterCommitFlush);
    let op = RedoOp::Write {
        path: "/p6.txt".into(),
        data: b"p6-host".to_vec(),
    };
    assert_eq!(
        j.run_transaction(&mut fs, core::slice::from_ref(&op)),
        Err(JournalError::InjectedCrash)
    );
    assert!(j.p6_lines.iter().any(|l| l.starts_with("P6_ARMED")));
    assert!(j
        .p6_lines
        .iter()
        .any(|l| l.starts_with("P6_TRANSACTION_COMMITTED")));

    let dev = j.into_device();
    let mut j2 = PhaseFsJournal::attach(dev).unwrap();
    let mut fs2 = PhaseFs::new();
    assert_eq!(j2.recover(&mut fs2), Err(JournalError::InjectedCrash));
    assert!(j2
        .p6_lines
        .iter()
        .any(|l| l.contains("P6_RECOVERY_BEGIN")));
    assert!(j2
        .p6_lines
        .iter()
        .any(|l| l.contains("P6_RECOVERY_CRASH_FIRED")));

    let dev = j2.into_device();
    let mut j3 = PhaseFsJournal::attach(dev).unwrap();
    let mut fs3 = PhaseFs::new();
    let r = j3.recover(&mut fs3).unwrap();
    assert_eq!(r.class, RecoverClass::Recovered);
    assert_eq!(fs3.read_file("/p6.txt").unwrap(), b"p6-host");
    assert!(!j3
        .p6_lines
        .iter()
        .any(|l| l.contains("P6_RECOVERY_CRASH_FIRED")));

    let (mut fs4, r2, _) = recover_from(j3);
    assert_eq!(r2.class, RecoverClass::Clean);
    assert_eq!(fs4.read_file("/p6.txt").unwrap(), b"p6-host");
}

#[test]
fn p6_corrupt_plan_does_not_fire() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    let run = j.next_txn_id();
    j.arm_p6(run).unwrap();
    j.set_crash(CrashPoint::AfterCommitFlush);
    let _ = j.run_transaction(
        &mut fs,
        &[RedoOp::Write {
            path: "/x.txt".into(),
            data: b"x".to_vec(),
        }],
    );
    let mut dev = j.into_device();
    dev.corrupt(crate::fs::journal::P6_PLAN_REL, 8, 0xFF);
    let mut j2 = PhaseFsJournal::attach(dev).unwrap();
    let mut fs2 = PhaseFs::new();
    let r = j2.recover(&mut fs2).unwrap();
    assert_eq!(r.class, RecoverClass::Recovered);
    assert!(!j2
        .p6_lines
        .iter()
        .any(|l| l.contains("P6_RECOVERY_CRASH_FIRED")));
    assert_eq!(fs2.read_file("/x.txt").unwrap(), b"x");
}

#[test]
fn p6_disarmed_recover_unchanged() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    j.set_crash(CrashPoint::AfterCommitFlush);
    let _ = j.run_transaction(
        &mut fs,
        &[RedoOp::Write {
            path: "/d.txt".into(),
            data: b"d".to_vec(),
        }],
    );
    let (mut fs2, report, _) = recover_from(j);
    assert_eq!(report.class, RecoverClass::Recovered);
    assert_eq!(fs2.read_file("/d.txt").unwrap(), b"d");
}

#[test]
fn p6_mismatched_txn_does_not_fire() {
    use super::p6_plan::{decode_plan, encode_plan};
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    let run = j.next_txn_id();
    j.arm_p6(run).unwrap();
    j.set_crash(CrashPoint::AfterCommitFlush);
    let _ = j.run_transaction(
        &mut fs,
        &[RedoOp::Write {
            path: "/m.txt".into(),
            data: b"m".to_vec(),
        }],
    );
    let mut dev = j.into_device();
    let mut sec = [0u8; 512];
    dev.read(crate::fs::journal::P6_PLAN_REL, &mut sec).unwrap();
    let mut p = decode_plan(&sec).unwrap();
    p.txn_id = p.txn_id.wrapping_add(99);
    let enc = encode_plan(&p);
    dev.write(crate::fs::journal::P6_PLAN_REL, &enc).unwrap();
    let mut j2 = PhaseFsJournal::attach(dev).unwrap();
    let mut fs2 = PhaseFs::new();
    let r = j2.recover(&mut fs2).unwrap();
    assert_eq!(r.class, RecoverClass::Recovered);
    assert!(!j2
        .p6_lines
        .iter()
        .any(|l| l.contains("P6_RECOVERY_CRASH_FIRED")));
    assert_eq!(fs2.read_file("/m.txt").unwrap(), b"m");
}

fn p7_win() -> P7Window {
    P7Window {
        start_lba: 15706112,
        p1_sectors: 16384,
        device_sectors: 15728640,
        p0_start: 2048,
        p0_sectors: 15704064,
    }
}

struct IgnoreWrites(RamBlockDev);

impl BlockDevice for IgnoreWrites {
    fn read(&mut self, lba: u32, buf: &mut [u8; 512]) -> Result<(), crate::fs::blockdev::BlockDevError> {
        self.0.read(lba, buf)
    }
    fn write(&mut self, _lba: u32, _buf: &[u8; 512]) -> Result<(), crate::fs::blockdev::BlockDevError> {
        Ok(())
    }
    fn flush(&mut self) -> Result<(), crate::fs::blockdev::BlockDevError> {
        Ok(())
    }
}

struct FlushFail(RamBlockDev);

impl BlockDevice for FlushFail {
    fn read(&mut self, lba: u32, buf: &mut [u8; 512]) -> Result<(), crate::fs::blockdev::BlockDevError> {
        self.0.read(lba, buf)
    }
    fn write(&mut self, lba: u32, buf: &[u8; 512]) -> Result<(), crate::fs::blockdev::BlockDevError> {
        self.0.write(lba, buf)
    }
    fn flush(&mut self) -> Result<(), crate::fs::blockdev::BlockDevError> {
        Err(crate::fs::blockdev::BlockDevError::Io)
    }
}

#[test]
fn p7_refuses_wrong_token() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    assert_eq!(
        j.p7_inject(1, "WRONG", p7_win()).unwrap_err(),
        P7Fail::WrongToken
    );
}

#[test]
fn p7_refuses_no_final_wal_sector() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    assert_eq!(
        j.p7_corrupt_existing_tail(P7_TOKEN, p7_win()).unwrap_err(),
        P7Fail::NoWalSector
    );
}

#[test]
fn p7_refuses_committed_tail() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    j.set_crash(CrashPoint::AfterCommitFlush);
    let _ = j.run_transaction(
        &mut fs,
        &[
            RedoOp::Mkdir {
                path: "/tmp".into(),
            },
            RedoOp::Write {
                path: "/tmp/p7_anchor.txt".into(),
                data: b"p7-anchor-preserve".to_vec(),
            },
        ],
    );
    assert_eq!(
        j.p7_corrupt_existing_tail(P7_TOKEN, p7_win()).unwrap_err(),
        P7Fail::CommittedTail
    );
}

#[test]
fn p7_refuses_io_write() {
    let fs0 = PhaseFs::new();
    let j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut dev = j.into_device();
    dev.set_crash_after_writes(0);
    let mut j2 = PhaseFsJournal::attach(dev).unwrap();
    assert_eq!(j2.p7_inject(2, P7_TOKEN, p7_win()).unwrap_err(), P7Fail::Io);
}

#[test]
fn p7_refuses_flush_failure() {
    let fs0 = PhaseFs::new();
    let j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut j2 = PhaseFsJournal::attach(FlushFail(j.into_device())).unwrap();
    assert_eq!(j2.p7_inject(2, P7_TOKEN, p7_win()).unwrap_err(), P7Fail::Io);
}

#[test]
fn p7_refuses_readback_mismatch() {
    let fs0 = PhaseFs::new();
    let j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut j2 = PhaseFsJournal::attach(IgnoreWrites(j.into_device())).unwrap();
    assert_eq!(
        j2.p7_inject(2, P7_TOKEN, p7_win()).unwrap_err(),
        P7Fail::ReadbackMismatch
    );
}

#[test]
fn p7_inject_rejects_corrupt_preserves_anchor() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    j.run_transaction(
        &mut fs,
        &[
            RedoOp::Mkdir {
                path: "/tmp".into(),
            },
            RedoOp::Write {
                path: "/tmp/p7_anchor.txt".into(),
                data: b"p7-anchor-preserve".to_vec(),
            },
        ],
    )
    .unwrap();
    let report = j.p7_inject(9, P7_TOKEN, p7_win()).unwrap();
    assert!(report.abs_lba >= 15706112);
    assert!(report.abs_lba < 15706112 + 16384);
    assert_ne!(report.before, report.after);
    let torn = "/tmp/p7_torn_9.txt";
    assert!(j.p6_lines.iter().any(|l| l.starts_with("P7_ARMED run=9")));
    assert!(j
        .p6_lines
        .iter()
        .any(|l| l.contains("P7_CORRUPTION_VERIFIED run=9")));
    let (mut fs2, r1, j2) = recover_from(j);
    assert_eq!(r1.class, RecoverClass::RejectedCorrupt);
    assert!(!r1.replayed);
    assert_eq!(fs2.read_file("/tmp/p7_anchor.txt").unwrap(), b"p7-anchor-preserve");
    assert!(fs2.read_file(torn).is_err());
    let (mut fs3, r2, j3) = recover_from(j2);
    assert_eq!(r2.class, RecoverClass::Clean);
    assert_eq!(fs3.read_file("/tmp/p7_anchor.txt").unwrap(), b"p7-anchor-preserve");
    assert!(fs3.read_file(torn).is_err());
    let (mut fs4, r3, _) = recover_from(j3);
    assert_eq!(r3.class, RecoverClass::Clean);
    assert_eq!(fs4.read_file("/tmp/p7_anchor.txt").unwrap(), b"p7-anchor-preserve");
}

#[test]
fn media_unavailable_display_is_not_journal_io() {
    assert_eq!(
        format!("{}", JournalError::MediaUnavailable),
        "media unavailable"
    );
    assert_eq!(format!("{}", JournalError::Io), "journal i/o");
    let d = crate::fs::blockdev::BlockIoDiag {
        op: crate::fs::blockdev::IO_OP_READ,
        lba: 15706112,
        count: 1,
        present: 3,
        nistr: 0,
        eistr: 0,
        class: crate::fs::blockdev::IO_CLASS_INHIBIT,
        reinit: 1,
        head: 8,
        tail: 8,
        generation: 1,
    };
    let s = format!("{}", JournalError::IoDetail(d));
    assert!(s.starts_with("journal i/o op=read lba=15706112"));
    assert!(s.contains("class=inhibit"));
    assert!(s.contains("reinit=1"));
}

#[test]
fn p9_classifier_accepts_historical_p9dbg_pass_and_rejects_io() {
    let pass = include_str!("../../../fpga/zynq/research/phasefs-persistent-journal-v1/com12_fsjournal_p9_20260815_190631.txt");
    assert_eq!(classify_p9_evidence(pass, false), P9Verdict::Pass);
    assert_eq!(classify_p9_evidence(pass, true), P9Verdict::FailP9dbg);
    let io = "gpos>\nfsj recover: journal i/o\ngpos>\n";
    assert_eq!(classify_p9_evidence(io, false), P9Verdict::FailIo);
    let clean = "gpos>\nfsj recover: media unavailable\nbackend: NONE\ndurable: NO\ngpos>\n";
    assert_eq!(classify_p9_evidence(clean, true), P9Verdict::Pass);
}

#[test]
fn p8_wrap_commit_survives_recover() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    let (h0, h1, _u) = j.p8_wrap_commit(&mut fs, 8).unwrap();
    assert_ne!(h0, h1);
    assert!(j.p6_lines.iter().any(|l| l.contains("P8_WRAPPED run=8")));
    assert_eq!(fs.read_file("/tmp/p8_wrap_8.txt").unwrap(), b"p8-wrap-8");
    let (mut fs2, report, _) = recover_from(j);
    assert_eq!(fs2.read_file("/tmp/p8_wrap_8.txt").unwrap(), b"p8-wrap-8");
    assert!(fs2.read_file("/p8f519.txt").is_ok());
    assert_ne!(report.class, RecoverClass::RejectedCorrupt);
}

#[test]
fn p10_uncommitted_is_isolated_anchor_kept() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    j.p10_stage_uncommitted(&mut fs, 41, P10_TOKEN).unwrap();
    assert_eq!(fs.read_file(P10_ANCHOR_PATH).unwrap(), P10_ANCHOR_BYTES);
    assert!(fs.read_file("/tmp/p10_target_41.txt").is_err());
    let (mut fs2, report, _) = recover_from(j);
    assert_eq!(report.class, RecoverClass::Isolated);
    assert!(!report.replayed);
    assert!(report.discarded);
    assert_eq!(fs2.read_file(P10_ANCHOR_PATH).unwrap(), P10_ANCHOR_BYTES);
    assert!(fs2.read_file("/tmp/p10_target_41.txt").is_err());
    assert_eq!(
        classify_p10_boot("fsj recover: class=ISOLATED replayed=0 discarded=1 gen=3", 41),
        P10Verdict::Pass
    );
}

#[test]
fn p10_wrong_token() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    assert_eq!(
        j.p10_stage_uncommitted(&mut fs, 1, "nope"),
        Err(P10Fail::WrongToken)
    );
}

#[test]
fn p11_commit_survives_recover() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    let txn = j.p11_commit(&mut fs, 7, P11_TOKEN).unwrap();
    assert!(txn > 0);
    assert_eq!(fs.read_file("/tmp/p11_target_7.txt").unwrap(), b"p11-must-survive-7");
    let (mut fs2, report, _) = recover_from(j);
    assert_ne!(report.class, RecoverClass::Isolated);
    assert_eq!(fs2.read_file("/tmp/p11_target_7.txt").unwrap(), b"p11-must-survive-7");
    assert_eq!(
        classify_p11_boot("p11-must-survive-7\nfsj check: PASS\n", 7),
        P11Verdict::Pass
    );
}

#[test]
fn p11_wrong_token() {
    let fs0 = PhaseFs::new();
    let mut j = PhaseFsJournal::format_ram(&fs0).unwrap();
    let mut fs = PhaseFs::new();
    assert_eq!(j.p11_commit(&mut fs, 1, "nope"), Err(P11Fail::WrongToken));
}

#[test]
fn wal_success_does_not_fall_back_to_txlog() {
    assert!(!wal_unavailable_falls_back_to_txlog(Ok(())));
}

#[test]
fn wal_no_partition_and_media_unavailable_fall_back_to_txlog() {
    assert!(wal_unavailable_falls_back_to_txlog(Err(
        JournalError::NoPartition
    )));
    assert!(wal_unavailable_falls_back_to_txlog(Err(
        JournalError::MediaUnavailable
    )));
}

#[test]
fn wal_real_errors_do_not_fall_back_to_txlog() {
    assert!(!wal_unavailable_falls_back_to_txlog(Err(
        JournalError::Checksum
    )));
    assert!(!wal_unavailable_falls_back_to_txlog(Err(JournalError::Io)));
    assert!(!wal_unavailable_falls_back_to_txlog(Err(
        JournalError::JournalFull
    )));
}
