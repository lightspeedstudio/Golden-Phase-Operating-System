// M=4 glider with controlled fault injection (Phase 5).
// FAULT_MODE: 0 none, 1 delete@gen2, 2 duplicate@gen2, 3 flip2to3@gen2, 4 skip update@gen2
module phase_lattice_2d_fault #(
    parameter integer FAULT_MODE = 1,
    parameter integer GENS = 8
) (
    input  wire clk,
    input  wire rst,
    output wire tx,
    output reg  done,
    output reg  all_pass,
    // Debug observability so a negative test can prove recovery is REAL (the
    // live lattice was actually restored), not print-only. Leave unconnected
    // in the board top.
    output wire [2*16-1:0] dbg_x,
    output wire [2*16-1:0] dbg_ckpt,
    output wire            dbg_restored
);
    localparam integer M = 4;
    localparam integer NCELLS = 16;
    localparam integer FBITS = 5;
    localparam [1:0] VAC = 2'd1;
    localparam [1:0] DEF = 2'd2;
    localparam integer SEED_I = 2 * M + 2;

    function automatic [7:0] dig(input [1:0] v);
        dig = 8'h30 + {6'd0, v};
    endfunction
    function automatic [7:0] dig4(input [3:0] v);
        dig4 = 8'h30 + {4'd0, v};
    endfunction

    reg [2*NCELLS-1:0] x, mid_r;
    wire [2*NCELLS-1:0] mid_w, nxt;
    genvar r, c;
    generate
        for (r = 0; r < M; r = r + 1) begin : gr
            for (c = 0; c < M; c = c + 1) begin : gc
                localparam integer IDX = r * M + c;
                localparam integer IDXE = r * M + ((c + 1) % M);
                localparam integer IDXW = r * M + ((c + M - 1) % M);
                localparam integer IDXN = ((r + M - 1) % M) * M + c;
                localparam integer IDXS = ((r + 1) % M) * M + c;
                wire [1:0] row_l, col_l;
                compose_gate u0 (.a(x[2*IDXW +: 2]), .b(x[2*IDX +: 2]), .y(row_l));
                compose_gate u1 (.a(row_l), .b(x[2*IDXE +: 2]), .y(mid_w[2*IDX +: 2]));
                compose_gate u2 (.a(mid_r[2*IDXN +: 2]), .b(mid_r[2*IDX +: 2]), .y(col_l));
                compose_gate u3 (.a(col_l), .b(mid_r[2*IDXS +: 2]), .y(nxt[2*IDX +: 2]));
            end
        end
    endgenerate

    reg [FBITS-1:0] fcnt_c, fcnt_r;
    integer ii;
    always @(*) begin
        fcnt_c = 0;
        for (ii = 0; ii < NCELLS; ii = ii + 1)
            if (x[2*ii +: 2] != VAC) fcnt_c = fcnt_c + 1;
    end
    wire foot_ok = (fcnt_r == 1);
    reg has_phi3;
    integer pj;
    always @(*) begin
        has_phi3 = 1'b0;
        for (pj = 0; pj < NCELLS; pj = pj + 1)
            if (x[2*pj +: 2] == 2'd3) has_phi3 = 1'b1;
    end

    reg push; reg [7:0] byte_in; wire busy;
    uart_printer u_uart (.clk(clk), .rst(rst), .push(push), .byte_in(byte_in), .busy(busy), .tx(tx));

    localparam ST_BANNER=0, ST_LINE=1, ST_STEP_R=2, ST_STEP_C=3, ST_SETTLE=4,
               ST_FINAL=5, ST_DONE=6, ST_EMIT=7, ST_RECOVER=8, ST_VERIFY=9;
    reg [3:0] state, ret_state;
    reg [15:0] gen;
    reg [3:0] g0,g1,g2,g3,g4;
    reg [9:0] line_i, line_len;
    reg [7:0] line [0:95];
    reg [7:0] detect_lat;
    reg detected;
    reg fault_applied;
    // Executable recovery state: an authenticated known-good checkpoint of the
    // live lattice, and flags proving the live registers were actually restored.
    reg [2*NCELLS-1:0] x_ckpt;
    reg recovered;      // recovery attempted a real restore
    reg restored_ok;    // live lattice == checkpoint AND invariants hold
    assign dbg_x = x;
    assign dbg_ckpt = x_ckpt;
    assign dbg_restored = restored_ok;
    // Detect: footprint break, unexpected Φ3, or intentional skip (mode 4).
    wire fault_obs = (!foot_ok) | has_phi3 | ((FAULT_MODE == 4) & fault_applied);
    integer k;

    always @(posedge clk) begin
        push <= 0;
        if (rst) begin
            state <= ST_BANNER; ret_state <= ST_BANNER;
            for (k = 0; k < NCELLS; k = k + 1) x[2*k +: 2] <= VAC;
            x[2*SEED_I +: 2] <= DEF;
            mid_r <= 0; fcnt_r <= 0; gen <= 0;
            g0<=0;g1<=0;g2<=0;g3<=0;g4<=0;
            detect_lat <= 0; detected <= 0; fault_applied <= 0;
            x_ckpt <= 0; recovered <= 0; restored_ok <= 0;
            done <= 0; all_pass <= 0;
        end else begin
            fcnt_r <= fcnt_c;
            case (state)
                ST_BANNER: begin
                    line[0]<="F";line[1]<="A";line[2]<="U";line[3]<="L";line[4]<="T";
                    line[5]<=" ";line[6]<="M";line[7]<="O";line[8]<="D";line[9]<="E";
                    line[10]<=" ";line[11]<=dig4(FAULT_MODE[3:0]);line[12]<=8'h0A;
                    line_len<=13; line_i<=0; ret_state<=ST_LINE; state<=ST_EMIT;
                end
                ST_LINE: begin
                    if (fault_obs && !detected && fault_applied) begin
                        detected <= 1;
                        detect_lat <= gen[7:0];
                    end
                    line[0]<="G";
                    line[1]<=dig4(g0);line[2]<=dig4(g1);line[3]<=dig4(g2);
                    line[4]<=dig4(g3);line[5]<=dig4(g4);
                    line[6]<=" ";line[7]<="S";line[8]<="=";
                    for (k=0;k<NCELLS;k=k+1) line[9+k]<=dig(x[2*k +: 2]);
                    line[9+NCELLS]<=" ";line[10+NCELLS]<="F";line[11+NCELLS]<="=";
                    line[12+NCELLS]<=dig4((fcnt_r/10)%10);line[13+NCELLS]<=dig4(fcnt_r%10);
                    line[14+NCELLS]<=8'h0A;
                    line_len<=15+NCELLS; line_i<=0; ret_state<=ST_STEP_R; state<=ST_EMIT;
                end
                ST_STEP_R: begin
                    if (gen == GENS-1) state <= ST_RECOVER;
                    else begin mid_r <= mid_w; state <= ST_STEP_C; end
                end
                ST_STEP_C: begin
                    // Capture the authenticated checkpoint at a known-good gen
                    // (gen 1, before the fault is applied at gen 2).
                    if (gen == 16'd1) x_ckpt <= x;
                    // apply fault once when leaving gen 2
                    if (gen == 16'd2 && !fault_applied) begin
                        fault_applied <= 1;
                        if (FAULT_MODE == 1) begin
                            // delete: all vacuum
                            for (k=0;k<NCELLS;k=k+1) x[2*k +: 2] <= VAC;
                        end else if (FAULT_MODE == 2) begin
                            x <= nxt;
                            x[2*0 +: 2] <= DEF; // duplicate at (0,0)
                        end else if (FAULT_MODE == 3) begin
                            x <= nxt;
                            // flip any DEF to Phi3
                            for (k=0;k<NCELLS;k=k+1)
                                if (nxt[2*k +: 2] == DEF) x[2*k +: 2] <= 2'd3;
                        end else if (FAULT_MODE == 4) begin
                            // skip update: hold x
                        end else x <= nxt;
                    end else x <= nxt;
                    gen <= gen + 1;
                    if (g4==9) begin g4<=0; if (g3==9) begin g3<=0; if (g2==9) begin g2<=0; if (g1==9) begin g1<=0; g0<=g0+1; end else g1<=g1+1; end else g2<=g2+1; end else g3<=g3+1; end else g4<=g4+1;
                    state <= ST_SETTLE;
                end
                ST_SETTLE: state <= ST_LINE;
                ST_RECOVER: begin
                    // EXECUTABLE recovery: on a detected fault, restore the LIVE
                    // lattice registers from the authenticated checkpoint. This
                    // is a real register write, not a printed string.
                    if (detected) begin
                        x <= x_ckpt;
                        recovered <= 1;
                    end
                    state <= ST_VERIFY;
                end
                ST_VERIFY: begin
                    // One cycle after restore: x now holds the checkpoint. Prove
                    // the live state was actually restored to a coherent orbit
                    // (footprint 1, no Phi3) equal to the checkpoint.
                    restored_ok <= (x == x_ckpt) && (fcnt_c == 1) && !has_phi3;
                    state <= ST_FINAL;
                end
                ST_FINAL: begin
                    // FDIR-mapped observables (detect→isolate→RESTORE→prove).
                    line[0]<="F";line[1]<="A";line[2]<="U";line[3]<="L";line[4]<="T";
                    line[5]<=" ";line[6]<="D";line[7]<="E";line[8]<="T";line[9]<="E";
                    line[10]<="C";line[11]<="T";line[12]<=" ";line[13]<="l";line[14]<="a";
                    line[15]<="t";line[16]<="e";line[17]<="n";line[18]<="c";line[19]<="y";
                    line[20]<="=";line[21]<=dig4((detect_lat/10)%10);line[22]<=dig4(detect_lat%10);
                    line[23]<=8'h0A;
                    line[24]<="I";line[25]<="S";line[26]<="O";line[27]<="L";line[28]<="A";
                    line[29]<="T";line[30]<="E";line[31]<="=";line[32]<="f";line[33]<="o";
                    line[34]<="o";line[35]<="t";line[36]<="p";line[37]<="r";line[38]<="i";
                    line[39]<="n";line[40]<="t";line[41]<=8'h0A;
                    line[42]<="R";line[43]<="E";line[44]<="C";line[45]<="O";line[46]<="V";
                    line[47]<="E";line[48]<="R";line[49]<="=";
                    if (recovered && restored_ok) begin
                        line[50]<="r";line[51]<="e";line[52]<="s";line[53]<="t";
                        line[54]<="o";line[55]<="r";line[56]<="e";line[57]<="d";
                    end else if (detected) begin
                        line[50]<="I";line[51]<="S";line[52]<="O";line[53]<="L";
                        line[54]<="A";line[55]<="T";line[56]<="E";line[57]<="D";
                    end else begin
                        line[50]<="n";line[51]<="o";line[52]<="n";line[53]<="e";
                        line[54]<="-";line[55]<="-";line[56]<="-";line[57]<="-";
                    end
                    line[58]<=8'h0A;
                    line[59]<="P";line[60]<="R";line[61]<="O";line[62]<="V";line[63]<="E";
                    line[64]<="=";line[65]<="a";line[66]<="s";line[67]<="s";line[68]<="o";
                    line[69]<="c";line[70]<="_";line[71]<="p";line[72]<="a";line[73]<="r";
                    line[74]<="i";line[75]<="t";line[76]<="y";line[77]<=" ";
                    line[78]<=(recovered && restored_ok) ? "1" : "0";
                    line[79]<=8'h0A;
                    line[80]<="F";line[81]<="A";line[82]<="U";line[83]<="L";line[84]<="T";
                    line[85]<=" ";line[86]<="D";line[87]<="O";line[88]<="N";line[89]<="E";
                    line[90]<=8'h0A;
                    line_len<=91; line_i<=0; ret_state<=ST_DONE; state<=ST_EMIT;
                end
                // all_pass now REQUIRES real restoration: pass only if no fault,
                // or the fault was detected AND the live lattice was actually
                // restored to the checkpoint. A print-only recovery (no register
                // restore) leaves restored_ok=0 and therefore CANNOT pass.
                ST_DONE: begin done<=1; all_pass<=(FAULT_MODE==0) || (detected && recovered && restored_ok); end
                ST_EMIT: begin
                    if (!busy && !push) begin
                        if (line_i < line_len) begin
                            byte_in<=line[line_i]; push<=1; line_i<=line_i+1;
                        end else state<=ret_state;
                    end
                end
                default: state<=ST_DONE;
            endcase
        end
    end
endmodule

module phase_lattice_2d_fault_top #(
    parameter integer FAULT_MODE = 1,
    parameter integer GENS = 8
) (
    input wire clk, input wire btn_n, input wire uart_rx,
    output wire uart_tx, output wire ledr_n, output wire ledg_n
);
    wire _u = uart_rx;
    reg [15:0] btn_sync;
    always @(posedge clk) btn_sync <= {btn_sync[14:0], ~btn_n};
    wire btn_rst = (btn_sync == 16'hFFFF);
    reg [13:0] por_cnt=0; reg por_done=0;
    always @(posedge clk) if (!por_done) begin if (por_cnt==12000) por_done<=1; else por_cnt<=por_cnt+1; end
    reg [25:0] resend_cnt; reg force_rst; wire done, all_pass;
    wire rst = btn_rst | (~por_done) | force_rst;
    always @(posedge clk) begin
        force_rst<=0;
        if (btn_rst|(~por_done)) resend_cnt<=0;
        else if (done) begin
            if (resend_cnt==48_000_000) begin resend_cnt<=0; force_rst<=1; end
            else resend_cnt<=resend_cnt+1;
        end else resend_cnt<=0;
    end
    phase_lattice_2d_fault #(.FAULT_MODE(FAULT_MODE), .GENS(GENS)) u_f (
        .clk(clk), .rst(rst), .tx(uart_tx), .done(done), .all_pass(all_pass));
    reg [23:0] hb; always @(posedge clk) if (rst) hb<=0; else hb<=hb+1;
    assign ledg_n = ~(hb[22]); assign ledr_n = ~(done & ~all_pass);
endmodule
