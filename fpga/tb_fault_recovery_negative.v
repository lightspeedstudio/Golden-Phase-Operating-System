// tb_fault_recovery_negative.v
// NEGATIVE TEST (HOST_UNIT_TEST): proves the corrected phase_lattice_2d_fault
// performs REAL recovery, and that a print-only recovery (RECOVER=observed with
// no register restore) CANNOT pass.
//
// It drives the module through a real fault and asserts, via the debug taps,
// that the LIVE lattice registers were actually written back to the
// authenticated checkpoint (dbg_x == dbg_ckpt) and restored_ok is set. A
// print-only implementation never writes x, so dbg_x != dbg_ckpt and
// restored_ok stays 0 -> this test FAILS. Thus "RECOVER=observed" cannot pass.
`timescale 1ns/1ps

module tb_fault_recovery_negative;
    parameter integer FAULT_MODE = `FAULT_MODE_SEL;

    reg clk = 0, rst = 1;
    always #5 clk = ~clk;

    wire tx, done, all_pass;
    wire [31:0] dbg_x, dbg_ckpt;
    wire dbg_restored;

    phase_lattice_2d_fault #(.FAULT_MODE(FAULT_MODE), .GENS(8)) dut (
        .clk(clk), .rst(rst), .tx(tx), .done(done), .all_pass(all_pass),
        .dbg_x(dbg_x), .dbg_ckpt(dbg_ckpt), .dbg_restored(dbg_restored)
    );

    integer cyc; integer fails;
    initial begin
        fails = 0;
        #12 rst = 0;
        // The FSM advances through the real UART emitter (~1000 cyc/byte), so
        // reaching `done` takes hundreds of thousands of cycles.
        for (cyc = 0; cyc < 2000000 && !done; cyc = cyc + 1) @(posedge clk);

        $display("== NEGATIVE TEST: fault_mode=%0d ==", FAULT_MODE);
        $display("done=%0b all_pass=%0b restored=%0b", done, all_pass, dbg_restored);
        $display("dbg_x   =%08h", dbg_x);
        $display("dbg_ckpt=%08h", dbg_ckpt);

        if (!done) begin $display("FAIL: never finished"); fails=fails+1; end

        if (FAULT_MODE == 0) begin
            // no fault: must pass, no restoration needed
            if (!all_pass) begin $display("FAIL: clean run must pass"); fails=fails+1; end
        end else begin
            // fault injected: the ONLY way to pass is REAL restoration.
            if (!dbg_restored) begin
                $display("FAIL: recovery not proven real (restored_ok=0) -- a print-only recovery lands here");
                fails=fails+1;
            end
            if (dbg_x !== dbg_ckpt) begin
                $display("FAIL: live lattice NOT restored to checkpoint (print-only recovery cannot pass)");
                fails=fails+1;
            end
            if (!all_pass) begin
                $display("FAIL: all_pass=0 -- module did not achieve real restoration");
                fails=fails+1;
            end
        end

        if (fails == 0) $display("RESULT: PASS (mode %0d) [HOST_UNIT_TEST] -- recovery is REAL", FAULT_MODE);
        else            $display("RESULT: FAIL (mode %0d) fails=%0d [HOST_UNIT_TEST]", FAULT_MODE, fails);
        $finish;
    end
endmodule
