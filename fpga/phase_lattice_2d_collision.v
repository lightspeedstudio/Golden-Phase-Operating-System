// Phase 3 — M=4 two-glider / domain collision harness.
// rule_a = colpass(rowpass). SCENARIO selects fixed seed layout.
// Host: verify_lattice_2d_collision.py

module phase_lattice_2d_collision #(
    parameter integer SCENARIO = 0, // 0 head-on, 1 phase-offset, 2 vs-reflect, 3 domain
    parameter integer GENS     = 16
) (
    input  wire clk,
    input  wire rst,
    output wire tx,
    output reg  done,
    output reg  all_pass
);
    localparam integer M = 4;
    localparam integer NCELLS = 16;
    localparam integer LINE_MAX = 80;
    localparam integer FBITS = 5;
    localparam [1:0] VAC = 2'd1;

    function automatic [7:0] dig(input [1:0] v);
        dig = 8'h30 + {6'd0, v};
    endfunction
    function automatic [7:0] dig4(input [3:0] v);
        dig4 = 8'h30 + {4'd0, v};
    endfunction

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

    reg [FBITS-1:0] fcnt_c, fcnt_r;
    integer ii;
    always @(*) begin
        fcnt_c = {FBITS{1'b0}};
        for (ii = 0; ii < NCELLS; ii = ii + 1)
            if (x[2*ii +: 2] != VAC)
                fcnt_c = fcnt_c + {{(FBITS-1){1'b0}}, 1'b1};
    end
    wire [3:0] f_t = (fcnt_r / 10) % 10;
    wire [3:0] f_o = fcnt_r % 10;

    reg        push;
    reg  [7:0] byte_in;
    wire       busy;
    uart_printer u_uart (.clk(clk), .rst(rst), .push(push), .byte_in(byte_in), .busy(busy), .tx(tx));

    localparam ST_BANNER = 3'd0;
    localparam ST_LINE   = 3'd1;
    localparam ST_STEP_R = 3'd2;
    localparam ST_STEP_C = 3'd3;
    localparam ST_SETTLE = 3'd4;
    localparam ST_FINAL  = 3'd5;
    localparam ST_DONE   = 3'd6;
    localparam ST_EMIT   = 3'd7;

    reg [2:0]  state, ret_state;
    reg [15:0] gen;
    reg [3:0]  g_hi, g_lo;
    reg [9:0]  line_i, line_len;
    reg [7:0]  line [0:LINE_MAX-1];
    reg [FBITS-1:0] m_before;
    integer k;

    task automatic seed_grid;
        integer cc;
        begin
            for (cc = 0; cc < NCELLS; cc = cc + 1)
                x[2*cc +: 2] <= VAC;
            if (SCENARIO == 0) begin
                x[2*(0*M+0) +: 2] <= 2'd2;
                x[2*(0*M+2) +: 2] <= 2'd2;
            end else if (SCENARIO == 1) begin
                x[2*(0*M+0) +: 2] <= 2'd2;
                x[2*(0*M+1) +: 2] <= 2'd2;
            end else if (SCENARIO == 2) begin
                x[2*(2*M+2) +: 2] <= 2'd2;
                x[2*(0*M+0) +: 2] <= 2'd3;
            end else begin
                x[2*(0*M+0) +: 2] <= 2'd2;
                for (cc = 0; cc < M; cc = cc + 1)
                    x[2*(2*M+cc) +: 2] <= 2'd3;
            end
        end
    endtask

    always @(posedge clk) begin
        push <= 1'b0;
        if (rst) begin
            state <= ST_BANNER; ret_state <= ST_BANNER;
            seed_grid;
            mid_r <= {(2*NCELLS){1'b0}};
            fcnt_r <= {FBITS{1'b0}};
            gen <= 16'd0; g_hi <= 4'd0; g_lo <= 4'd0;
            m_before <= {FBITS{1'b0}};
            line_i <= 10'd0; line_len <= 10'd0;
            done <= 1'b0; all_pass <= 1'b0;
        end else begin
            fcnt_r <= fcnt_c;
            case (state)
                ST_BANNER: begin
                    line[0]<="C"; line[1]<="O"; line[2]<="L"; line[3]<="L";
                    line[4]<="I"; line[5]<="S"; line[6]<="I"; line[7]<="O";
                    line[8]<="N"; line[9]<=" "; line[10]<="S"; line[11]<=dig4(SCENARIO[3:0]);
                    line[12]<=" "; line[13]<="M"; line[14]<="4"; line[15]<=8'h0A;
                    line_len <= 10'd16; line_i <= 10'd0;
                    ret_state <= ST_LINE; state <= ST_EMIT;
                end
                ST_LINE: begin
                    if (gen == 16'd0) m_before <= fcnt_r;
                    line[0]<="G"; line[1]<=dig4(g_hi); line[2]<=dig4(g_lo);
                    line[3]<=" "; line[4]<="S"; line[5]<="=";
                    for (k = 0; k < NCELLS; k = k + 1)
                        line[6+k] <= dig(x[2*k +: 2]);
                    line[6+NCELLS]<=" "; line[6+NCELLS+1]<="F"; line[6+NCELLS+2]<="=";
                    line[6+NCELLS+3]<=dig4(f_t); line[6+NCELLS+4]<=dig4(f_o);
                    line[6+NCELLS+5]<=8'h0A;
                    line_len <= 6 + NCELLS + 6; line_i <= 10'd0;
                    ret_state <= ST_STEP_R; state <= ST_EMIT;
                end
                ST_STEP_R: begin
                    if (gen == GENS - 1) state <= ST_FINAL;
                    else begin mid_r <= mid_w; state <= ST_STEP_C; end
                end
                ST_STEP_C: begin
                    x <= nxt;
                    gen <= gen + 16'd1;
                    if (g_lo == 4'd9) begin g_lo <= 4'd0; g_hi <= g_hi + 4'd1; end
                    else g_lo <= g_lo + 4'd1;
                    state <= ST_SETTLE;
                end
                ST_SETTLE: state <= ST_LINE;
                ST_FINAL: begin
                    // CLASS line filled by host; FPGA reports Mb/Ma and DONE
                    line[0]<="M"; line[1]<="b"; line[2]<="=";
                    line[3]<=dig4((m_before/10)%10); line[4]<=dig4(m_before%10);
                    line[5]<=" "; line[6]<="M"; line[7]<="a"; line[8]<="=";
                    line[9]<=dig4((fcnt_r/10)%10); line[10]<=dig4(fcnt_r%10);
                    line[11]<=8'h0A;
                    line[12]<="C"; line[13]<="O"; line[14]<="L"; line[15]<="L";
                    line[16]<="I"; line[17]<="S"; line[18]<="I"; line[19]<="O";
                    line[20]<="N"; line[21]<=" "; line[22]<="D"; line[23]<="O";
                    line[24]<="N"; line[25]<="E"; line[26]<=8'h0A;
                    line_len <= 10'd27; line_i <= 10'd0;
                    ret_state <= ST_DONE; state <= ST_EMIT;
                end
                ST_DONE: begin done <= 1'b1; all_pass <= 1'b1; end
                ST_EMIT: begin
                    if (!busy && !push) begin
                        if (line_i < line_len) begin
                            byte_in <= line[line_i];
                            push <= 1'b1;
                            line_i <= line_i + 10'd1;
                        end else state <= ret_state;
                    end
                end
                default: state <= ST_DONE;
            endcase
        end
    end
endmodule

module phase_lattice_2d_collision_top #(
    parameter integer SCENARIO = 0,
    parameter integer GENS     = 16
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
    wire       done, all_pass;
    wire       rst = btn_rst | (~por_done) | force_rst;

    always @(posedge clk) begin
        force_rst <= 1'b0;
        if (btn_rst | (~por_done)) resend_cnt <= 26'd0;
        else if (done) begin
            if (resend_cnt == 26'd48_000_000) begin
                resend_cnt <= 26'd0;
                force_rst <= 1'b1;
            end else resend_cnt <= resend_cnt + 26'd1;
        end else resend_cnt <= 26'd0;
    end

    phase_lattice_2d_collision #(.SCENARIO(SCENARIO), .GENS(GENS)) u_col (
        .clk(clk), .rst(rst), .tx(uart_tx), .done(done), .all_pass(all_pass)
    );

    reg [23:0] hb;
    always @(posedge clk) begin
        if (rst) hb <= 24'd0;
        else hb <= hb + 24'd1;
    end
    assign ledg_n = ~(hb[22]);
    assign ledr_n = ~(done & ~all_pass);
    wire _u = |{_unused_rx};
endmodule
