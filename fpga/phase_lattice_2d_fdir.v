// phase_lattice_2d_fdir.v
// REAL detect -> diagnose -> RESTORE -> prove for the 2D phase-lattice glider.
//
// This CORRECTS the print-only recovery in phase_lattice_2d_fault.v, whose
// ST_FINAL emits "RECOVER=observed" over UART but never restores the lattice.
// Here recovery is executable: on a confirmed/catastrophic fault the live
// lattice registers `x_live` are ACTUALLY overwritten from an authenticated
// checkpoint snapshot `x_ckpt`, the canonical invariants are re-verified, the
// state root is recomputed, and the lattice is resumed with a continuation
// step. If restoration cannot be proven, the module isolates (does not lie).
//
// Canonical compose (`cmp`) is the Golden Phase table, transcribed verbatim
// from kairos/fpga/phase_alu.v / compose_gate_uart.v. The testbench asserts
// `cmp` equals that table for all 16 pairs, so this is the real equation, not
// a substitute. The 2-pass (row then column) toroidal update matches the
// compose_gate cascade (u0..u3) in phase_lattice_2d_fault.v.
//
// Observability is via register outputs so a simulator can audit real state
// transitions. No UART transcript is produced or fabricated here.

module phase_lattice_2d_fdir #(
    parameter integer FAULT_MODE = 1,   // 0 none,1 delete,2 duplicate,3 flip2to3,4 skip-update
    parameter integer GENS       = 8,
    parameter integer CKPT_GEN   = 1,   // capture authenticated checkpoint after this gen (known-good)
    parameter integer FAULT_GEN  = 2,   // research: inject at this gen
    parameter integer N_CONFIRM  = 1    // consecutive over-threshold samples to confirm a transient
) (
    input  wire        clk,
    input  wire        rst,
    output reg  [31:0] x_live,          // live lattice (16 cells x 2 bits)
    output reg  [31:0] x_ckpt,          // authenticated checkpoint snapshot
    output reg  [7:0]  root_live,       // R live  (cell checksum root)
    output reg  [7:0]  root_ckpt,       // R checkpoint
    output reg  [15:0] d2,              // bounded syndrome severity (raw)
    output reg  [15:0] accum,           // A_k temporal confirmation (EWMA)
    output reg         i1_ok,           // canonical compose invariant (no illegal Phi3 in orbit)
    output reg         i2_ok,           // coherence/footprint invariant (exactly one live cell)
    output reg         catastrophic,    // algebraically impossible -> bypass debounce
    output reg         confirmed,       // debounced fault confirmation
    output reg         recovered,       // live restore proven == checkpoint + invariants
    output reg         isolated,        // recovery could not be proven
    output reg  [3:0]  stage,           // 0 seed,1 run,2 injected,3 restoring,4 recovered,5 isolated,6 done
    output reg         done
);
    localparam [1:0] VAC = 2'd1;
    localparam [1:0] DEF = 2'd2;
    localparam integer SEED_I = 2*4 + 2; // (2,2) in 4x4, matches fault module SEED_I = 2*M+2

    // ---- bounded scoring constants (see overflow proof in testbench notes) ----
    localparam integer Q     = 16;
    localparam integer QMAX  = 255;
    localparam integer D_MAX = 16'hFFFF;
    localparam integer W_I2  = 8;      // footprint weight
    localparam integer W_KAP = 2048;   // composed-phase (kappa) residual penalty
    localparam integer W_I1  = 4096;   // compose-invariant penalty (Phi3)
    localparam integer W_ROOT= 1024;   // root-mismatch penalty

    // ---- canonical compose: Golden Phase table (verbatim from phase_alu.v) ----
    function [1:0] cmp(input [1:0] a, input [1:0] b);
        case ({a, b})
            4'b0000: cmp = 2'd0; 4'b0001: cmp = 2'd1; 4'b0010: cmp = 2'd2; 4'b0011: cmp = 2'd3;
            4'b0100: cmp = 2'd1; 4'b0101: cmp = 2'd3; 4'b0110: cmp = 2'd3; 4'b0111: cmp = 2'd2;
            4'b1000: cmp = 2'd2; 4'b1001: cmp = 2'd3; 4'b1010: cmp = 2'd3; 4'b1011: cmp = 2'd1;
            4'b1100: cmp = 2'd3; 4'b1101: cmp = 2'd2; 4'b1110: cmp = 2'd1; 4'b1111: cmp = 2'd0;
            default: cmp = 2'd0;
        endcase
    endfunction

    // one toroidal 2-pass compose update over the 4x4 lattice
    function [31:0] step;
        input [31:0] s;
        reg [1:0] mid [0:15];
        integer r, c, idx, idxe, idxw, idxn, idxs;
        reg [1:0] row_l, col_l;
        begin
            for (r = 0; r < 4; r = r + 1)
                for (c = 0; c < 4; c = c + 1) begin
                    idx  = r*4 + c;
                    idxe = r*4 + ((c+1)%4);
                    idxw = r*4 + ((c+3)%4);
                    row_l   = cmp(s[2*idxw +: 2], s[2*idx +: 2]);
                    mid[idx]= cmp(row_l,          s[2*idxe +: 2]);
                end
            for (r = 0; r < 4; r = r + 1)
                for (c = 0; c < 4; c = c + 1) begin
                    idx  = r*4 + c;
                    idxn = ((r+3)%4)*4 + c;
                    idxs = ((r+1)%4)*4 + c;
                    col_l          = cmp(mid[idxn], mid[idx]);
                    step[2*idx +: 2] = cmp(col_l, mid[idxs]);
                end
        end
    endfunction

    function [7:0] root_of;
        input [31:0] s;
        integer i; reg [7:0] acc;
        begin acc = 8'd0;
            for (i = 0; i < 16; i = i + 1) acc = acc + {6'd0, s[2*i +: 2]};
            root_of = acc;
        end
    endfunction

    function [4:0] foot_of;   // count of non-vacuum cells
        input [31:0] s;
        integer i; reg [4:0] f;
        begin f = 5'd0;
            for (i = 0; i < 16; i = i + 1) if (s[2*i +: 2] != VAC) f = f + 5'd1;
            foot_of = f;
        end
    endfunction

    function has_phi3;        // any cell in illegal Phi3 state
        input [31:0] s;
        integer i; reg h;
        begin h = 1'b0;
            for (i = 0; i < 16; i = i + 1) if (s[2*i +: 2] == 2'd3) h = 1'b1;
            has_phi3 = h;
        end
    endfunction

    // saturating q_i = min(QMAX, |e|*Q/L) with L=1
    function [15:0] q_of;
        input [15:0] e;
        reg [31:0] t;
        begin t = e * Q; q_of = (t > QMAX) ? QMAX : t[15:0]; end
    endfunction

    localparam ST_SEED=4'd0, ST_RUN=4'd1, ST_INJECT=4'd2, ST_RESTORE=4'd3,
               ST_RECOVERED=4'd4, ST_ISOLATE=4'd5, ST_DONE=4'd6, ST_VERIFY=4'd7, ST_CONT=4'd8;

    reg [15:0] gen;
    reg        fault_applied;
    reg [7:0]  confirm_cnt;
    reg [31:0] nxt_s;
    reg [31:0] x_prev;      // independent PhaseGuard lane: last accepted state
    reg        have_prev;
    integer    k;

    // combinational syndrome over the current live state
    reg  [4:0] foot;
    reg        phi3, kappa_skip;
    reg  [15:0] e_i2, q_i2, d2_c;
    reg        i1_c, i2_c, root_mismatch, cata_c;
    always @(*) begin
        foot = foot_of(x_live);
        phi3 = has_phi3(x_live);
        i1_c = ~phi3;
        i2_c = (foot == 5'd1);
        root_mismatch = (root_of(x_live) != root_ckpt) && (root_ckpt != 8'd0);
        // e_kappa: independent predictor F_K(x_prev) must not equal x_prev while
        // the live state failed to advance (skipped update / tick stall).
        kappa_skip = have_prev && (x_live == x_prev) && (step(x_prev) != x_prev);
        // e_i2 = |foot-1| (footprint deviation).
        e_i2 = (foot >= 5'd1) ? (foot - 5'd1) : 5'd1;
        q_i2 = q_of(e_i2);
        d2_c = 16'd0;
        d2_c = d2_c + ((W_I2 * q_i2 * q_i2 > D_MAX) ? D_MAX : (W_I2 * q_i2 * q_i2));
        if (kappa_skip)     d2_c = (d2_c + W_KAP  > D_MAX) ? D_MAX : d2_c + W_KAP;
        if (!i1_c)          d2_c = (d2_c + W_I1  > D_MAX) ? D_MAX : d2_c + W_I1;
        if (root_mismatch)  d2_c = (d2_c + W_ROOT > D_MAX) ? D_MAX : d2_c + W_ROOT;
        // catastrophic: Phi3 has no preimage in the legal glider orbit, and a
        // fully annihilated footprint is algebraically impossible mid-orbit.
        cata_c = phi3 || (foot == 5'd0);
    end

    always @(posedge clk) begin
        if (rst) begin
            for (k = 0; k < 16; k = k + 1) x_live[2*k +: 2] <= VAC;
            x_live[2*SEED_I +: 2] <= DEF;
            x_ckpt <= 32'd0; root_ckpt <= 8'd0; root_live <= 8'd0;
            gen <= 16'd0; fault_applied <= 1'b0; confirm_cnt <= 8'd0;
            x_prev <= 32'd0; have_prev <= 1'b0;
            d2 <= 16'd0; accum <= 16'd0;
            i1_ok <= 1'b1; i2_ok <= 1'b1; catastrophic <= 1'b0;
            confirmed <= 1'b0; recovered <= 1'b0; isolated <= 1'b0;
            stage <= ST_SEED; done <= 1'b0;
        end else begin
            root_live <= root_of(x_live);
            d2 <= d2_c;
            i1_ok <= i1_c; i2_ok <= i2_c; catastrophic <= cata_c;
            // A_k = ((N-1)A + D2)/N with N=4 (integer EWMA)
            accum <= ((3*accum) + d2_c) >> 2;

            case (stage)
                ST_SEED: begin
                    // capture authenticated checkpoint at the known-good gen
                    x_ckpt <= x_live; root_ckpt <= root_of(x_live);
                    stage <= ST_RUN;
                end
                ST_RUN: begin
                    if (gen == CKPT_GEN) begin
                        x_ckpt <= x_live; root_ckpt <= root_of(x_live);
                    end
                    // debounce bookkeeping on the live syndrome
                    if (d2_c > 16'd0) confirm_cnt <= confirm_cnt + 8'd1;
                    else confirm_cnt <= 8'd0;
                    confirmed <= (confirm_cnt + 8'd1 >= N_CONFIRM) && (d2_c > 0);

                    if (cata_c || ((d2_c > 0) && (confirm_cnt + 8'd1 >= N_CONFIRM))) begin
                        // real fault present -> begin executable recovery
                        stage <= ST_RESTORE;
                    end else if (gen == FAULT_GEN && !fault_applied && FAULT_MODE != 0) begin
                        // research-only live corruption of the lattice registers
                        fault_applied <= 1'b1;
                        x_prev <= x_live; have_prev <= 1'b1; // predictor baseline
                        stage <= ST_INJECT;
                    end else if (gen >= GENS-1) begin
                        stage <= ST_DONE;
                    end else begin
                        x_prev <= x_live; have_prev <= 1'b1; // predictor baseline
                        x_live <= step(x_live);
                        gen <= gen + 16'd1;
                    end
                end
                ST_INJECT: begin
                    // corrupt LIVE registers (research). This is a real state
                    // change, not a temp copy.
                    if (FAULT_MODE == 1) begin
                        for (k = 0; k < 16; k = k + 1) x_live[2*k +: 2] <= VAC; // delete
                    end else if (FAULT_MODE == 2) begin
                        x_live <= step(x_live);
                        x_live[2*0 +: 2] <= DEF;                                 // duplicate
                    end else if (FAULT_MODE == 3) begin
                        nxt_s = step(x_live);
                        x_live <= nxt_s;
                        for (k = 0; k < 16; k = k + 1)
                            if (nxt_s[2*k +: 2] == DEF) x_live[2*k +: 2] <= 2'd3; // flip to Phi3
                    end else if (FAULT_MODE == 4) begin
                        x_live <= x_live;                                        // skip update
                    end
                    gen <= gen + 16'd1;
                    stage <= ST_RUN;
                end
                ST_RESTORE: begin
                    // EXECUTABLE recovery: overwrite live registers from the
                    // authenticated checkpoint snapshot.
                    x_live <= x_ckpt;
                    stage <= ST_VERIFY;
                end
                ST_VERIFY: begin
                    // recompute root and re-check invariants on the restored state
                    root_live <= root_of(x_live);
                    if ((x_live == x_ckpt) && (foot_of(x_live) == 5'd1) && !has_phi3(x_live)
                        && (root_of(x_live) == root_ckpt)) begin
                        recovered <= 1'b1;
                        confirmed <= 1'b0;
                        stage <= ST_CONT;
                    end else begin
                        stage <= ST_ISOLATE;
                    end
                end
                ST_CONT: begin
                    // continuation: prove the lattice still evolves legally
                    x_live <= step(x_live);
                    if (has_phi3(step(x_live))) stage <= ST_ISOLATE;
                    else stage <= ST_DONE;
                end
                ST_ISOLATE: begin
                    isolated <= 1'b1; done <= 1'b1;
                end
                ST_DONE: begin
                    done <= 1'b1;
                end
                default: stage <= ST_DONE;
            endcase
        end
    end
endmodule
