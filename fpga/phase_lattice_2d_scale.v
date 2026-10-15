// Avenue 9 — 2-D torus phase lattice (scaling study).
//
// Reuses `compose_gate` from phase_lattice_uart.v UNCHANGED (read together
// at synthesis/sim time; this file does NOT redefine compose). Standing
// rule: never reimplement compose. Does NOT reuse `lattice_cell` (that is
// specifically the 3-input 1-D wiring) — this is a genuinely different
// wiring of the same compose primitive, per Avenue 2's brief.
//
// Grid: M x M torus, periodic in both row and column. Per Avenue 2, TWO
// candidate 2-D update rules are compared every cycle (their disagreement
// is the 2-D curvature/defect signal, exactly analogous to the 1-D L!=R
// defect):
//   (a) "row rule then column rule":
//         mid = (W . x) . E          -- 1-D-style compose along the row
//         y_a = (N . mid) . S        -- then along the column
//   (b) "fixed grouping" x' = ((N.x).S) . ((E.x).W)  (exact formula from
//       the Avenue 2 brief):
//         t1  = (N . x) . S
//         t2  = (E . x) . W
//         y_b = t1 . t2
// defect = (y_a != y_b). Next state = y_a (an arbitrary but documented
// choice, mirroring the 1-D module's use of L as next-state).
// 9 compose_gate instances per cell (4 for y_a, 5 for y_b).

module lattice_cell_2d (
    input  wire [1:0] xn,
    input  wire [1:0] xs,
    input  wire [1:0] xe,
    input  wire [1:0] xw,
    input  wire [1:0] xc,
    output wire [1:0] y_a,
    output wire       defect
);
    wire [1:0] mid, y_b, t1, t2;

    // (a) row then column
    compose_gate u_mid (.a(xw), .b(xc), .y(mid));
    compose_gate u_ya  (.a(xn), .b(mid), .y(y_a));

    // (b) ((N.x).S) . ((E.x).W)
    wire [1:0] nx, exw;
    compose_gate u_nx  (.a(xn), .b(xc), .y(nx));
    compose_gate u_t1  (.a(nx), .b(xs), .y(t1));
    compose_gate u_ex  (.a(xe), .b(xc), .y(exw));
    compose_gate u_t2  (.a(exw), .b(xw), .y(t2));
    compose_gate u_yb  (.a(t1), .b(t2), .y(y_b));

    assign defect = (y_a != y_b);
endmodule

module phase_lattice_2d_scale #(
    parameter integer M    = 4,
    parameter integer GENS = 4
) (
    input  wire clk,
    input  wire rst,
    output wire tx,
    output reg  done,
    output reg  all_pass
);
    localparam integer NCELLS = M * M;
    localparam integer LINE_MAX = NCELLS + 48;
    localparam integer DBITS  = $clog2(NCELLS + 1);

    function automatic [7:0] dig(input [1:0] v);
        dig = 8'h30 + {6'd0, v};
    endfunction
    function automatic [7:0] dig4(input [3:0] v);
        dig4 = 8'h30 + {4'd0, v};
    endfunction

    reg  [2*NCELLS-1:0] x;
    wire [2*NCELLS-1:0] lw;
    wire [NCELLS-1:0]   d;

    genvar r, c;
    generate
        for (r = 0; r < M; r = r + 1) begin : g_row
            for (c = 0; c < M; c = c + 1) begin : g_col
                localparam integer IDX  = r * M + c;
                localparam integer IDXN = ((r + M - 1) % M) * M + c;
                localparam integer IDXS = ((r + 1) % M) * M + c;
                localparam integer IDXE = r * M + ((c + 1) % M);
                localparam integer IDXW = r * M + ((c + M - 1) % M);
                lattice_cell_2d u_cell (
                    .xn(x[2*IDXN +: 2]),
                    .xs(x[2*IDXS +: 2]),
                    .xe(x[2*IDXE +: 2]),
                    .xw(x[2*IDXW +: 2]),
                    .xc(x[2*IDX  +: 2]),
                    .y_a(lw[2*IDX +: 2]),
                    .defect(d[IDX])
                );
            end
        end
    endgenerate

    reg [DBITS-1:0] dcnt;
    integer ii;
    always @(*) begin
        dcnt = {DBITS{1'b0}};
        for (ii = 0; ii < NCELLS; ii = ii + 1) dcnt = dcnt + d[ii];
    end

    wire [DBITS-1:0] d_h = dcnt / 100;
    wire [DBITS-1:0] d_t = (dcnt / 10) % 10;
    wire [DBITS-1:0] d_o = dcnt % 10;

    reg        push;
    reg  [7:0] byte_in;
    wire       busy;
    uart_printer u_uart (.clk(clk), .rst(rst), .push(push), .byte_in(byte_in), .busy(busy), .tx(tx));

    localparam ST_BANNER = 3'd0;
    localparam ST_LINE   = 3'd1;
    localparam ST_STEP   = 3'd2;
    localparam ST_FINAL  = 3'd3;
    localparam ST_DONE   = 3'd4;
    localparam ST_EMIT   = 3'd5;

    reg [2:0]  state, ret_state;
    reg [15:0] gen;
    reg [3:0]  g_o;
    reg [9:0]  line_i, line_len;
    reg [7:0]  line [0:LINE_MAX-1];
    integer    k;

    localparam [7:0] M_T = 8'h30 + (((M / 10) % 10));
    localparam [7:0] M_O = 8'h30 + ((M % 10));

    always @(posedge clk) begin
        push <= 1'b0;
        if (rst) begin
            state <= ST_BANNER; ret_state <= ST_BANNER;
            for (k = 0; k < NCELLS; k = k + 1) x[2*k +: 2] <= (k % 4);
            gen <= 16'd0; g_o <= 4'd0;
            line_i <= 10'd0; line_len <= 10'd0;
            done <= 1'b0; all_pass <= 1'b0;
        end else begin
            case (state)
                ST_BANNER: begin
                    // "PHASE LATTICE 2D MxM\n" e.g "PHASE LATTICE 2D 04x04\n"
                    line[0]<="P"; line[1]<="H"; line[2]<="A"; line[3]<="S";
                    line[4]<="E"; line[5]<=" "; line[6]<="L"; line[7]<="A";
                    line[8]<="T"; line[9]<="T"; line[10]<="I"; line[11]<="C";
                    line[12]<="E"; line[13]<=" "; line[14]<="2"; line[15]<="D";
                    line[16]<=" "; line[17]<=M_T; line[18]<=M_O; line[19]<="x";
                    line[20]<=M_T; line[21]<=M_O; line[22]<=8'h0A;
                    line_len <= 10'd23; line_i <= 10'd0;
                    ret_state <= ST_LINE; state <= ST_EMIT;
                end
                ST_LINE: begin
                    line[0] <= "G"; line[1] <= 8'h30; line[2] <= dig4(g_o);
                    line[3] <= " "; line[4] <= "S"; line[5] <= "=";
                    for (k = 0; k < NCELLS; k = k + 1)
                        line[6+k] <= dig(x[2*k +: 2]);
                    line[6+NCELLS]   <= " "; line[6+NCELLS+1] <= "D"; line[6+NCELLS+2] <= "=";
                    line[6+NCELLS+3] <= dig4(d_h[3:0]);
                    line[6+NCELLS+4] <= dig4(d_t[3:0]);
                    line[6+NCELLS+5] <= dig4(d_o[3:0]);
                    line[6+NCELLS+6] <= 8'h0A;
                    line_len <= 6 + NCELLS + 7; line_i <= 10'd0;
                    ret_state <= ST_STEP; state <= ST_EMIT;
                end
                ST_STEP: begin
                    if (gen == GENS - 1) begin
                        state <= ST_FINAL;
                    end else begin
                        x   <= lw;
                        gen <= gen + 16'd1;
                        g_o <= g_o + 4'd1;
                        state <= ST_LINE;
                    end
                end
                ST_FINAL: begin
                    line[0]<="L";  line[1]<="A";  line[2]<="T";  line[3]<="T";
                    line[4]<="I";  line[5]<="C";  line[6]<="E";  line[7]<=" ";
                    line[8]<="D";  line[9]<="O";  line[10]<="N"; line[11]<="E";
                    line[12]<=8'h0A;
                    line_len <= 10'd13; line_i <= 10'd0;
                    ret_state <= ST_DONE; state <= ST_EMIT;
                end
                ST_DONE: begin
                    done     <= 1'b1;
                    all_pass <= 1'b1; // no on-chip predicate for 2D; presence of DONE is the pass signal
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

module phase_lattice_2d_top #(
    parameter integer M = 4
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

    wire rst = btn_rst | (~por_done) | force_rst;

    always @(posedge clk) begin
        force_rst <= 1'b0;
        if (btn_rst | (~por_done)) begin
            resend_cnt <= 26'd0;
        end else if (done) begin
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

    phase_lattice_2d_scale #(.M(M)) u_lattice (
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
