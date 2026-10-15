// Avenue 2 glider on silicon — parameterized seal harness (Phase 1).
// rule_a = colpass(rowpass). Parameters: M, GENS, VACUUM, DEFECT, SEED_R, SEED_C.
// Host: verify_lattice_2d_glider.py (exact state every generation).
// FPGA on-chip check: footprint == 1 only (position sealed by host↔UART).

module phase_lattice_2d_glider #(
    parameter integer M       = 4,
    parameter integer GENS    = 8,
    parameter integer VACUUM  = 1,
    parameter integer DEFECT  = 2,
    parameter integer SEED_R  = 2,
    parameter integer SEED_C  = 2
) (
    input  wire clk,
    input  wire rst,
    output wire tx,
    output reg  done,
    output reg  all_pass
);
    localparam integer NCELLS  = M * M;
    localparam integer LINE_MAX = NCELLS + 64;
    localparam integer FBITS   = $clog2(NCELLS + 1);
    localparam integer SEED_I  = SEED_R * M + SEED_C;
    localparam [1:0] VAC = VACUUM[1:0];
    localparam [1:0] DEF = DEFECT[1:0];

    function automatic [7:0] dig(input [1:0] v);
        dig = 8'h30 + {6'd0, v};
    endfunction
    function automatic [7:0] dig4(input [3:0] v);
        dig4 = 8'h30 + {4'd0, v};
    endfunction

    // Pipelined Avenue 2 rule_a: rowpass(x) -> mid_r, then colpass(mid_r) -> nxt.
    reg  [2*NCELLS-1:0] x;
    reg  [2*NCELLS-1:0] mid_r;
    wire [2*NCELLS-1:0] mid_w;
    wire [2*NCELLS-1:0] nxt;

    genvar r, c;
    generate
        for (r = 0; r < M; r = r + 1) begin : g_row
            for (c = 0; c < M; c = c + 1) begin : g_col
                localparam integer IDX  = r * M + c;
                localparam integer IDXE = r * M + ((c + 1) % M);
                localparam integer IDXW = r * M + ((c + M - 1) % M);
                localparam integer IDXN = ((r + M - 1) % M) * M + c;
                localparam integer IDXS = ((r + 1) % M) * M + c;

                wire [1:0] row_l;
                compose_gate u_rw (.a(x[2*IDXW +: 2]), .b(x[2*IDX +: 2]), .y(row_l));
                compose_gate u_rm (.a(row_l), .b(x[2*IDXE +: 2]), .y(mid_w[2*IDX +: 2]));

                wire [1:0] col_l;
                compose_gate u_cn (.a(mid_r[2*IDXN +: 2]), .b(mid_r[2*IDX +: 2]), .y(col_l));
                compose_gate u_cm (.a(col_l), .b(mid_r[2*IDXS +: 2]), .y(nxt[2*IDX +: 2]));
            end
        end
    endgenerate

    // Footprint vs vacuum (registered — combo popcount into UART was a timing killer).
    reg [FBITS-1:0] fcnt_c;
    reg [FBITS-1:0] fcnt_r;
    integer ii;
    always @(*) begin
        fcnt_c = {FBITS{1'b0}};
        for (ii = 0; ii < NCELLS; ii = ii + 1)
            if (x[2*ii +: 2] != VAC)
                fcnt_c = fcnt_c + {{(FBITS-1){1'b0}}, 1'b1};
    end
    wire [3:0] f_t = (fcnt_r / 10) % 10;
    wire [3:0] f_o = fcnt_r % 10;
    wire       foot_ok = (fcnt_r == {{(FBITS-1){1'b0}}, 1'b1});

    reg        push;
    reg  [7:0] byte_in;
    wire       busy;
    uart_printer u_uart (.clk(clk), .rst(rst), .push(push), .byte_in(byte_in), .busy(busy), .tx(tx));

    localparam ST_BANNER  = 3'd0;
    localparam ST_LINE    = 3'd1;
    localparam ST_STEP_R  = 3'd2;
    localparam ST_STEP_C  = 3'd3;
    localparam ST_SETTLE  = 3'd4;
    localparam ST_FINAL   = 3'd5;
    localparam ST_DONE    = 3'd6;
    localparam ST_EMIT    = 3'd7;

    reg [2:0]  state, ret_state;
    reg [15:0] gen;
    // Registered BCD gen digits (5 digits → GENS up to 99999; no combo dividers).
    reg [3:0]  g0, g1, g2, g3, g4;
    reg [9:0]  line_i, line_len;
    reg [7:0]  line [0:LINE_MAX-1];
    reg        track_ok;
    integer    k;

    localparam [7:0] M_T = 8'h30 + (((M / 10) % 10));
    localparam [7:0] M_O = 8'h30 + ((M % 10));

    always @(posedge clk) begin
        push <= 1'b0;
        if (rst) begin
            state <= ST_BANNER; ret_state <= ST_BANNER;
            for (k = 0; k < NCELLS; k = k + 1) x[2*k +: 2] <= VAC;
            x[2*SEED_I +: 2] <= DEF;
            mid_r <= {(2*NCELLS){1'b0}};
            fcnt_r <= {FBITS{1'b0}};
            gen <= 16'd0;
            g0 <= 4'd0; g1 <= 4'd0; g2 <= 4'd0; g3 <= 4'd0; g4 <= 4'd0;
            line_i <= 10'd0; line_len <= 10'd0;
            done <= 1'b0; all_pass <= 1'b0; track_ok <= 1'b1;
        end else begin
            fcnt_r <= fcnt_c;
            case (state)
                ST_BANNER: begin
                    // PHASE LATTICE 2D GLIDER MxM VdD Ss,s\n
                    line[0]<="P"; line[1]<="H"; line[2]<="A"; line[3]<="S";
                    line[4]<="E"; line[5]<=" "; line[6]<="L"; line[7]<="A";
                    line[8]<="T"; line[9]<="T"; line[10]<="I"; line[11]<="C";
                    line[12]<="E"; line[13]<=" "; line[14]<="2"; line[15]<="D";
                    line[16]<=" "; line[17]<="G"; line[18]<="L"; line[19]<="I";
                    line[20]<="D"; line[21]<="E"; line[22]<="R"; line[23]<=" ";
                    line[24]<=M_T; line[25]<=M_O; line[26]<="x";
                    line[27]<=M_T; line[28]<=M_O;
                    line[29]<=" "; line[30]<="V"; line[31]<=dig(VAC);
                    line[32]<="D"; line[33]<=dig(DEF);
                    line[34]<=" "; line[35]<="S";
                    line[36]<=dig4(SEED_R[3:0]); line[37]<=",";
                    line[38]<=dig4(SEED_C[3:0]);
                    line[39]<=8'h0A;
                    line_len <= 10'd40; line_i <= 10'd0;
                    ret_state <= ST_LINE; state <= ST_EMIT;
                end
                ST_LINE: begin
                    if (!foot_ok) track_ok <= 1'b0;
                    line[0] <= "G";
                    line[1] <= dig4(g0); line[2] <= dig4(g1);
                    line[3] <= dig4(g2); line[4] <= dig4(g3);
                    line[5] <= dig4(g4);
                    line[6] <= " "; line[7] <= "S"; line[8] <= "=";
                    for (k = 0; k < NCELLS; k = k + 1)
                        line[9+k] <= dig(x[2*k +: 2]);
                    line[9+NCELLS]   <= " ";
                    line[9+NCELLS+1] <= "F";
                    line[9+NCELLS+2] <= "=";
                    line[9+NCELLS+3] <= dig4(f_t);
                    line[9+NCELLS+4] <= dig4(f_o);
                    line[9+NCELLS+5] <= 8'h0A;
                    line_len <= 9 + NCELLS + 6; line_i <= 10'd0;
                    ret_state <= ST_STEP_R; state <= ST_EMIT;
                end
                ST_STEP_R: begin
                    if (gen == GENS - 1) begin
                        state <= ST_FINAL;
                    end else begin
                        mid_r <= mid_w;
                        state <= ST_STEP_C;
                    end
                end
                ST_STEP_C: begin
                    x   <= nxt;
                    gen <= gen + 16'd1;
                    // BCD increment g0..g4 (10000s .. ones)
                    if (g4 == 4'd9) begin
                        g4 <= 4'd0;
                        if (g3 == 4'd9) begin
                            g3 <= 4'd0;
                            if (g2 == 4'd9) begin
                                g2 <= 4'd0;
                                if (g1 == 4'd9) begin
                                    g1 <= 4'd0;
                                    if (g0 == 4'd9) g0 <= 4'd0;
                                    else g0 <= g0 + 4'd1;
                                end else g1 <= g1 + 4'd1;
                            end else g2 <= g2 + 4'd1;
                        end else g3 <= g3 + 4'd1;
                    end else g4 <= g4 + 4'd1;
                    state <= ST_SETTLE;
                end
                ST_SETTLE: begin
                    state <= ST_LINE;
                end
                ST_FINAL: begin
                    if (track_ok) begin
                        line[0]<="G"; line[1]<="L"; line[2]<="I"; line[3]<="D";
                        line[4]<="E"; line[5]<="R"; line[6]<=" "; line[7]<="P";
                        line[8]<="A"; line[9]<="S"; line[10]<="S"; line[11]<=8'h0A;
                    end else begin
                        line[0]<="G"; line[1]<="L"; line[2]<="I"; line[3]<="D";
                        line[4]<="E"; line[5]<="R"; line[6]<=" "; line[7]<="F";
                        line[8]<="A"; line[9]<="I"; line[10]<="L"; line[11]<=8'h0A;
                    end
                    line[12]<="L"; line[13]<="A"; line[14]<="T"; line[15]<="T";
                    line[16]<="I"; line[17]<="C"; line[18]<="E"; line[19]<=" ";
                    line[20]<="D"; line[21]<="O"; line[22]<="N"; line[23]<="E";
                    line[24]<=8'h0A;
                    line_len <= 10'd25; line_i <= 10'd0;
                    ret_state <= ST_DONE; state <= ST_EMIT;
                end
                ST_DONE: begin
                    done     <= 1'b1;
                    all_pass <= track_ok;
                end
                ST_EMIT: begin
                    if (!busy && !push) begin
                        if (line_i < line_len) begin
                            byte_in <= line[line_i];
                            push    <= 1'b1;
                            line_i  <= line_i + 10'd1;
                        end else state <= ret_state;
                    end
                end
                default: state <= ST_DONE;
            endcase
        end
    end
endmodule

module phase_lattice_2d_glider_top #(
    parameter integer M       = 4,
    parameter integer GENS    = 8,
    parameter integer VACUUM  = 1,
    parameter integer DEFECT  = 2,
    parameter integer SEED_R  = 2,
    parameter integer SEED_C  = 2
) (
    input  wire clk,
    input  wire btn_n,
    input  wire uart_rx,
    output wire uart_tx,
    output wire ledr_n,
    output wire ledg_n
);
    wire _unused_rx = uart_rx;

    reg [15:0] btn_sync;
    always @(posedge clk) btn_sync <= {btn_sync[14:0], ~btn_n};
    wire btn_rst = (btn_sync == 16'hFFFF);

    reg [13:0] por_cnt = 14'd0;
    reg        por_done = 1'b0;
    always @(posedge clk) begin
        if (!por_done) begin
            if (por_cnt == 14'd12000) por_done <= 1'b1;
            else por_cnt <= por_cnt + 14'd1;
        end
    end

    reg [25:0] resend_cnt;
    reg        force_rst;
    wire       done;
    wire       all_pass;
    wire       rst = btn_rst | (~por_done) | force_rst;

    always @(posedge clk) begin
        force_rst <= 1'b0;
        if (btn_rst | (~por_done)) begin
            resend_cnt <= 26'd0;
        end else if (done) begin
            // ~4 s @ 12 MHz — re-emit so host capture cannot miss the burst
            if (resend_cnt == 26'd48_000_000) begin
                resend_cnt <= 26'd0;
                force_rst  <= 1'b1;
            end else begin
                resend_cnt <= resend_cnt + 26'd1;
            end
        end else begin
            resend_cnt <= 26'd0;
        end
    end

    phase_lattice_2d_glider #(
        .M(M), .GENS(GENS), .VACUUM(VACUUM), .DEFECT(DEFECT),
        .SEED_R(SEED_R), .SEED_C(SEED_C)
    ) u_glider (
        .clk(clk),
        .rst(rst),
        .tx(uart_tx),
        .done(done),
        .all_pass(all_pass)
    );

    reg [23:0] hb_cnt;
    always @(posedge clk) begin
        if (rst) hb_cnt <= 24'd0;
        else hb_cnt <= hb_cnt + 24'd1;
    end
    assign ledg_n = ~(hb_cnt[22]);
    assign ledr_n = ~(done & ~all_pass);

    wire _unused = |{_unused_rx};
endmodule
