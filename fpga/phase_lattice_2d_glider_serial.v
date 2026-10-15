// Row/column-serial Avenue-2 glider for M=8 timing.
// Each cycle updates one full row (rowpass) or one full column (colpass).
// Host: verify_lattice_2d_glider.py

module phase_lattice_2d_glider_serial #(
    parameter integer M       = 8,
    parameter integer GENS    = 16,
    parameter integer VACUUM  = 1,
    parameter integer DEFECT  = 2,
    parameter integer SEED_R  = 4,
    parameter integer SEED_C  = 4
) (
    input  wire clk,
    input  wire rst,
    output wire tx,
    output reg  done,
    output reg  all_pass
);
    localparam integer NCELLS = M * M;
    localparam integer LINE_MAX = NCELLS + 64;
    localparam integer FBITS = $clog2(NCELLS + 1);
    localparam integer SEED_I = SEED_R * M + SEED_C;
    localparam [1:0] VAC = VACUUM[1:0];
    localparam [1:0] DEF = DEFECT[1:0];

    function automatic [7:0] dig(input [1:0] v);
        dig = 8'h30 + {6'd0, v};
    endfunction
    function automatic [7:0] dig4(input [3:0] v);
        dig4 = 8'h30 + {4'd0, v};
    endfunction

    reg [1:0] x   [0:NCELLS-1];
    reg [1:0] mid [0:NCELLS-1];
    reg [1:0] nxt [0:NCELLS-1];

    reg [FBITS-1:0] fcnt_r;
    reg [FBITS-1:0] fcnt_acc;
    // Registered BCD digits — avoid /10 on any path into UART assemble.
    reg [3:0] f_t, f_o;
    wire foot_ok = (fcnt_r == {{(FBITS-1){1'b0}}, 1'b1});

    reg push; reg [7:0] byte_in; wire busy;
    uart_printer u_uart (.clk(clk), .rst(rst), .push(push), .byte_in(byte_in), .busy(busy), .tx(tx));

    localparam ST_BANNER=0, ST_LINE=1, ST_ROW=2, ST_COL=3, ST_COMMIT=4, ST_COUNT=5, ST_SETTLE=6, ST_FINAL=7, ST_DONE=8, ST_EMIT=9;
    reg [3:0] state, ret_state;
    reg [15:0] gen;
    reg [3:0] g0,g1,g2,g3,g4;
    reg [9:0] line_i, line_len;
    reg [7:0] line [0:LINE_MAX-1];
    reg [5:0] scan; // cell index for count; low bits = row/col index
    reg track_ok;
    integer k, c, r;

    localparam [7:0] M_T = 8'h30 + (((M / 10) % 10));
    localparam [7:0] M_O = 8'h30 + ((M % 10));

    // Combinational rowpass for current scan row
    wire [1:0] row_y [0:M-1];
    genvar gc;
    generate
        for (gc = 0; gc < M; gc = gc + 1) begin : g_rowcell
            wire [1:0] xv = x[scan * M + gc];
            wire [1:0] xw = x[scan * M + ((gc + M - 1) % M)];
            wire [1:0] xe = x[scan * M + ((gc + 1) % M)];
            wire [1:0] rl;
            compose_gate u_rw (.a(xw), .b(xv), .y(rl));
            compose_gate u_rm (.a(rl), .b(xe), .y(row_y[gc]));
        end
    endgenerate

    // Combinational colpass for current scan column
    wire [1:0] col_y [0:M-1];
    generate
        for (gc = 0; gc < M; gc = gc + 1) begin : g_colcell
            wire [1:0] mv = mid[gc * M + scan];
            wire [1:0] mn = mid[((gc + M - 1) % M) * M + scan];
            wire [1:0] ms = mid[((gc + 1) % M) * M + scan];
            wire [1:0] cl;
            compose_gate u_cn (.a(mn), .b(mv), .y(cl));
            compose_gate u_cm (.a(cl), .b(ms), .y(col_y[gc]));
        end
    endgenerate

    always @(posedge clk) begin
        push <= 1'b0;
        if (rst) begin
            state <= ST_BANNER; ret_state <= ST_BANNER;
            for (k = 0; k < NCELLS; k = k + 1) x[k] <= VAC;
            x[SEED_I] <= DEF;
            for (k = 0; k < NCELLS; k = k + 1) begin mid[k] <= 2'd0; nxt[k] <= 2'd0; end
            fcnt_r <= 0; fcnt_acc <= 0; f_t <= 0; f_o <= 0; gen <= 0; scan <= 0;
            g0<=0;g1<=0;g2<=0;g3<=0;g4<=0;
            done <= 0; all_pass <= 0; track_ok <= 1;
        end else begin
            case (state)
                ST_BANNER: begin
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
                    line[38]<=dig4(SEED_C[3:0]); line[39]<=8'h0A;
                    line_len <= 40; line_i <= 0;
                    fcnt_acc <= 0; scan <= 0;
                    ret_state <= ST_COUNT; state <= ST_EMIT;
                end
                ST_LINE: begin
                    if (!foot_ok) track_ok <= 0;
                    line[0]<="G";
                    line[1]<=dig4(g0); line[2]<=dig4(g1); line[3]<=dig4(g2);
                    line[4]<=dig4(g3); line[5]<=dig4(g4);
                    line[6]<=" "; line[7]<="S"; line[8]<="=";
                    for (k = 0; k < NCELLS; k = k + 1) line[9+k] <= dig(x[k]);
                    line[9+NCELLS]<=" "; line[10+NCELLS]<="F"; line[11+NCELLS]<="=";
                    line[12+NCELLS]<=dig4(f_t); line[13+NCELLS]<=dig4(f_o);
                    line[14+NCELLS]<=8'h0A;
                    line_len <= 15+NCELLS; line_i <= 0; ret_state <= ST_ROW; state <= ST_EMIT;
                    scan <= 0;
                end
                ST_ROW: begin
                    if (gen == GENS - 1) state <= ST_FINAL;
                    else begin
                        for (c = 0; c < M; c = c + 1)
                            mid[scan * M + c] <= row_y[c];
                        if (scan[2:0] == M[2:0] - 3'd1) begin scan <= 0; state <= ST_COL; end
                        else scan <= scan + 1;
                    end
                end
                ST_COL: begin
                    for (r = 0; r < M; r = r + 1)
                        nxt[r * M + scan] <= col_y[r];
                    if (scan[2:0] == M[2:0] - 3'd1) state <= ST_COMMIT;
                    else scan <= scan + 1;
                end
                ST_COMMIT: begin
                    for (k = 0; k < NCELLS; k = k + 1) x[k] <= nxt[k];
                    gen <= gen + 1;
                    if (g4==9) begin g4<=0;
                        if (g3==9) begin g3<=0;
                            if (g2==9) begin g2<=0;
                                if (g1==9) begin g1<=0; g0<=g0+1; end
                                else g1<=g1+1;
                            end else g2<=g2+1;
                        end else g3<=g3+1;
                    end else g4<=g4+1;
                    fcnt_acc <= 0; scan <= 0; state <= ST_COUNT;
                end
                ST_COUNT: begin
                    // one cell per clock; latch total on last index
                    if (scan == NCELLS[5:0] - 6'd1) begin
                        fcnt_r <= fcnt_acc + ((x[scan] != VAC) ? {{(FBITS-1){1'b0}}, 1'b1} : {FBITS{1'b0}});
                        state <= ST_SETTLE;
                    end else begin
                        fcnt_acc <= fcnt_acc + ((x[scan] != VAC) ? {{(FBITS-1){1'b0}}, 1'b1} : {FBITS{1'b0}});
                        scan <= scan + 6'd1;
                    end
                end
                ST_SETTLE: begin
                    // decimal digits for F=xx (footprint ≤ 64)
                    if (fcnt_r < 10) begin f_t <= 0; f_o <= fcnt_r[3:0]; end
                    else if (fcnt_r < 20) begin f_t <= 1; f_o <= fcnt_r - 6'd10; end
                    else if (fcnt_r < 30) begin f_t <= 2; f_o <= fcnt_r - 6'd20; end
                    else if (fcnt_r < 40) begin f_t <= 3; f_o <= fcnt_r - 6'd30; end
                    else if (fcnt_r < 50) begin f_t <= 4; f_o <= fcnt_r - 6'd40; end
                    else if (fcnt_r < 60) begin f_t <= 5; f_o <= fcnt_r - 6'd50; end
                    else begin f_t <= 6; f_o <= fcnt_r - 6'd60; end
                    state <= ST_LINE;
                end
                ST_FINAL: begin
                    if (track_ok) begin
                        line[0]<="G";line[1]<="L";line[2]<="I";line[3]<="D";
                        line[4]<="E";line[5]<="R";line[6]<=" ";line[7]<="P";
                        line[8]<="A";line[9]<="S";line[10]<="S";line[11]<=8'h0A;
                    end else begin
                        line[0]<="G";line[1]<="L";line[2]<="I";line[3]<="D";
                        line[4]<="E";line[5]<="R";line[6]<=" ";line[7]<="F";
                        line[8]<="A";line[9]<="I";line[10]<="L";line[11]<=8'h0A;
                    end
                    line[12]<="L";line[13]<="A";line[14]<="T";line[15]<="T";
                    line[16]<="I";line[17]<="C";line[18]<="E";line[19]<=" ";
                    line[20]<="D";line[21]<="O";line[22]<="N";line[23]<="E";
                    line[24]<=8'h0A;
                    line_len<=25; line_i<=0; ret_state<=ST_DONE; state<=ST_EMIT;
                end
                ST_DONE: begin done<=1; all_pass<=track_ok; end
                ST_EMIT: begin
                    if (!busy && !push) begin
                        if (line_i < line_len) begin
                            byte_in <= line[line_i]; push <= 1; line_i <= line_i + 1;
                        end else state <= ret_state;
                    end
                end
                default: state <= ST_DONE;
            endcase
        end
    end
endmodule

module phase_lattice_2d_glider_serial_top #(
    parameter integer M=8, parameter integer GENS=16,
    parameter integer VACUUM=1, parameter integer DEFECT=2,
    parameter integer SEED_R=4, parameter integer SEED_C=4
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
    phase_lattice_2d_glider_serial #(.M(M),.GENS(GENS),.VACUUM(VACUUM),.DEFECT(DEFECT),.SEED_R(SEED_R),.SEED_C(SEED_C))
        u_g (.clk(clk),.rst(rst),.tx(uart_tx),.done(done),.all_pass(all_pass));
    reg [23:0] hb; always @(posedge clk) if (rst) hb<=0; else hb<=hb+1;
    assign ledg_n=~(hb[22]); assign ledr_n=~(done & ~all_pass);
endmodule
