// Live one-packet UART smoke (Avenue 10B).
// Commands: I=inject, M=move ETA gens, A=ACK, P=PROP_ARM (after ACK).
// Carrier: sealed M=4 V1D2 glider rule_a. ETA (2,2)->(0,0) = 2 gens.
// Host: flash-capture-phase-lattice-2d-packet.ps1 + verify_lattice_2d_packet.py

module phase_lattice_2d_packet #(
    parameter integer GENS = 2
) (
    input  wire clk,
    input  wire rst,
    input  wire rx,
    output wire tx,
    output reg  done,
    output reg  all_pass
);
    localparam integer M = 4;
    localparam integer NCELLS = 16;
    localparam integer FBITS = 5;
    localparam [1:0] VAC = 2'd1;
    localparam [1:0] DEF = 2'd2;
    localparam integer SEED_I = 2 * M + 2; // (2,2)

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

    wire rx_valid; wire [7:0] rx_data;
    uart_rx u_rx (.clk(clk), .rst(rst), .rx(rx), .valid(rx_valid), .data(rx_data));

    reg push; reg [7:0] byte_in; wire busy;
    uart_printer u_uart (.clk(clk), .rst(rst), .push(push), .byte_in(byte_in), .busy(busy), .tx(tx));

    localparam ST_BANNER=0, ST_WAIT=1, ST_INJECT=2, ST_LINE=3, ST_STEP_R=4, ST_STEP_C=5,
               ST_SETTLE=6, ST_RECV=7, ST_ACK=8, ST_PROP=9, ST_DONE=10, ST_EMIT=11;
    reg [3:0] state, ret_state;
    reg [15:0] gen;
    reg [3:0] g0, g1;
    reg [9:0] line_i, line_len;
    reg [7:0] line [0:63];
    reg injected, moved, acked, prop_ok, track_ok;
    reg [24:0] wait_cnt;
    integer k;

    task automatic seed_vac;
        integer cc;
        begin
            for (cc = 0; cc < NCELLS; cc = cc + 1)
                x[2*cc +: 2] <= VAC;
        end
    endtask

    always @(posedge clk) begin
        push <= 1'b0;
        if (rst) begin
            state <= ST_BANNER; ret_state <= ST_BANNER;
            seed_vac; mid_r <= 0; fcnt_r <= 0;
            gen <= 0; g0 <= 0; g1 <= 0;
            injected <= 0; moved <= 0; acked <= 0; prop_ok <= 0; track_ok <= 1;
            wait_cnt <= 0;
            done <= 0; all_pass <= 0;
        end else begin
            fcnt_r <= fcnt_c;
            case (state)
                ST_BANNER: begin
                    line[0]<="P";line[1]<="K";line[2]<="T";line[3]<=" ";
                    line[4]<="R";line[5]<="E";line[6]<="A";line[7]<="D";
                    line[8]<="Y";line[9]<=8'h0A;
                    line_len<=10; line_i<=0; ret_state<=ST_WAIT; state<=ST_EMIT;
                    wait_cnt <= 0;
                end
                ST_WAIT: begin
                    wait_cnt <= wait_cnt + 1;
                    if (!injected && wait_cnt == 25'd12_000_000) begin
                        wait_cnt <= 0;
                        state <= ST_BANNER;
                    end else if (rx_valid) begin
                        if (rx_data == "I" || rx_data == "i") state <= ST_INJECT;
                        else if ((rx_data == "M" || rx_data == "m") && injected) begin
                            gen <= 0; g0 <= 0; g1 <= 0; state <= ST_LINE;
                        end else if ((rx_data == "A" || rx_data == "a") && moved && !acked)
                            state <= ST_ACK;
                        else if ((rx_data == "P" || rx_data == "p") && acked && !prop_ok)
                            state <= ST_PROP;
                    end
                end
                ST_INJECT: begin
                    seed_vac;
                    x[2*SEED_I +: 2] <= DEF;
                    injected <= 1;
                    line[0]<="P";line[1]<="K";line[2]<="T";line[3]<=" ";
                    line[4]<="I";line[5]<="N";line[6]<="J";line[7]<="E";
                    line[8]<="C";line[9]<="T";line[10]<=8'h0A;
                    line_len<=11; line_i<=0; ret_state<=ST_WAIT; state<=ST_EMIT;
                end
                ST_LINE: begin
                    if (!foot_ok) track_ok <= 0;
                    line[0]<="G"; line[1]<=dig4(g0); line[2]<=dig4(g1);
                    line[3]<=" "; line[4]<="S"; line[5]<="=";
                    for (k = 0; k < NCELLS; k = k + 1)
                        line[6+k] <= dig(x[2*k +: 2]);
                    line[6+NCELLS]<=" "; line[7+NCELLS]<="F"; line[8+NCELLS]<="=";
                    line[9+NCELLS]<=dig4((fcnt_r/10)%10); line[10+NCELLS]<=dig4(fcnt_r%10);
                    line[11+NCELLS]<=8'h0A;
                    line_len <= 12+NCELLS; line_i <= 0; ret_state <= ST_STEP_R; state <= ST_EMIT;
                end
                ST_STEP_R: begin
                    if (gen == GENS - 1) state <= ST_RECV;
                    else begin mid_r <= mid_w; state <= ST_STEP_C; end
                end
                ST_STEP_C: begin
                    x <= nxt;
                    gen <= gen + 1;
                    if (g1 == 9) begin g1 <= 0; g0 <= g0 + 1; end
                    else g1 <= g1 + 1;
                    state <= ST_SETTLE;
                end
                ST_SETTLE: state <= ST_LINE;
                ST_RECV: begin
                    moved <= 1;
                    line[0]<="P";line[1]<="K";line[2]<="T";line[3]<=" ";
                    line[4]<="R";line[5]<="E";line[6]<="C";line[7]<="V";
                    line[8]<=" ";line[9]<="S";line[10]<="=";
                    for (k = 0; k < NCELLS; k = k + 1)
                        line[11+k] <= dig(x[2*k +: 2]);
                    line[11+NCELLS]<=8'h0A;
                    line_len<=12+NCELLS; line_i<=0; ret_state<=ST_WAIT; state<=ST_EMIT;
                end
                ST_ACK: begin
                    acked <= 1;
                    line[0]<="P";line[1]<="K";line[2]<="T";line[3]<=" ";
                    line[4]<="A";line[5]<="C";line[6]<="K";line[7]<=8'h0A;
                    line_len<=8; line_i<=0; ret_state<=ST_WAIT; state<=ST_EMIT;
                end
                ST_PROP: begin
                    prop_ok <= 1;
                    line[0]<="P";line[1]<="K";line[2]<="T";line[3]<=" ";
                    line[4]<="P";line[5]<="R";line[6]<="O";line[7]<="P";
                    line[8]<="_";line[9]<="A";line[10]<="R";line[11]<="M";
                    line[12]<=" ";line[13]<="O";line[14]<="K";line[15]<=8'h0A;
                    line[16]<="P";line[17]<="K";line[18]<="T";line[19]<=" ";
                    line[20]<="D";line[21]<="O";line[22]<="N";line[23]<="E";
                    line[24]<=8'h0A;
                    line_len<=25; line_i<=0; ret_state<=ST_DONE; state<=ST_EMIT;
                end
                ST_DONE: begin
                    done <= 1;
                    all_pass <= track_ok & injected & moved & acked & prop_ok;
                end
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

module phase_lattice_2d_packet_top #(
    parameter integer GENS = 2
) (
    input wire clk, input wire btn_n, input wire uart_rx,
    output wire uart_tx, output wire ledr_n, output wire ledg_n
);
    reg [15:0] btn_sync;
    always @(posedge clk) btn_sync <= {btn_sync[14:0], ~btn_n};
    wire btn_rst = (btn_sync == 16'hFFFF);
    reg [13:0] por_cnt=0; reg por_done=0;
    always @(posedge clk) if (!por_done) begin if (por_cnt==12000) por_done<=1; else por_cnt<=por_cnt+1; end
    wire rst = btn_rst | (~por_done) | force_rst;
    wire done, all_pass;
    reg [25:0] resend_cnt = 0;
    reg force_rst = 0;
    always @(posedge clk) begin
        force_rst <= 0;
        if (btn_rst | (~por_done)) resend_cnt <= 0;
        else if (done) begin
            // Re-arm READY ~2s after DONE so soak / craft scripts need no DTR reset.
            if (resend_cnt == 26'd24_000_000) begin
                resend_cnt <= 0;
                force_rst <= 1;
            end else resend_cnt <= resend_cnt + 1;
        end else resend_cnt <= 0;
    end
    phase_lattice_2d_packet #(.GENS(GENS)) u_p (
        .clk(clk), .rst(rst), .rx(uart_rx), .tx(uart_tx), .done(done), .all_pass(all_pass));
    reg [23:0] hb; always @(posedge clk) if (rst) hb<=0; else hb<=hb+1;
    assign ledg_n = ~(hb[22]); assign ledr_n = ~(done & ~all_pass);
endmodule
