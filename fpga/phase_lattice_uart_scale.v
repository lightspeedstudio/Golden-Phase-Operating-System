// Avenue 9 — parameterized phase lattice SCALING STUDY (1-D ring).
//
// Reuses `compose_gate` and `lattice_cell` from phase_lattice_uart.v
// UNCHANGED (this file must be read together with phase_lattice_uart.v at
// synthesis/sim time; it does NOT redefine compose). Standing rule: never
// reimplement compose.
//
// This is a deliberately generalized sibling of phase_lattice_uart.v, NOT a
// replacement — the physically-verified N=16 experiment
// (phase_lattice_uart.v / phase_lattice_top.v / evidence/phase_lattice/) is
// untouched. Differences from that fixed experiment, made purely to get a
// clean N-generic resource/Fmax scaling study:
//   - Init field is x[i] = i % 4 (not the N=16 hand-picked field). Any fixed,
//     N-independent rule works for a resource study; i%4 is simplest and is
//     exactly what verify_lattice_scale.py replicates.
//   - The defect count field D is printed as 3 decimal digits (was 2) since
//     N can exceed 99.
//   - The banner prints N as 3 zero-padded decimal digits.
// Everything else (compose table, L-rule next-state, defect = L!=R,
// UART framing, Phi1 homogeneous-field 1/2 clock check) is the same idea as
// the verified experiment, just N-generic.

module phase_lattice_uart_scale #(
    parameter integer N        = 16,
    parameter integer GENS     = 8,
    parameter integer CLK_GENS = 4
) (
    input  wire clk,
    input  wire rst,
    output wire tx,
    output reg  done,
    output reg  all_pass
);
    localparam integer LINE_MAX = N + 48;
    localparam integer DBITS    = $clog2(N + 1);

    function automatic [7:0] dig(input [1:0] v);
        dig = 8'h30 + {6'd0, v};
    endfunction
    function automatic [7:0] dig4(input [3:0] v);
        dig4 = 8'h30 + {4'd0, v};
    endfunction

    // ------------------------------------------------------------------
    // The lattice: N cells x 4 compose gates each (reused lattice_cell)
    // ------------------------------------------------------------------
    reg  [2*N-1:0] x;
    wire [2*N-1:0] lw;
    wire [N-1:0]   d;

    genvar gi;
    generate
        for (gi = 0; gi < N; gi = gi + 1) begin : g_cells
            lattice_cell u_cell (
                .xm(x[2*((gi + N - 1) % N) +: 2]),
                .xc(x[2*gi +: 2]),
                .xp(x[2*((gi + 1) % N) +: 2]),
                .l_out(lw[2*gi +: 2]),
                .defect(d[gi])
            );
        end
    endgenerate

    // Combinational popcount of the defect vector -> dcnt (0..N)
    reg [DBITS-1:0] dcnt;
    integer ii;
    always @(*) begin
        dcnt = {DBITS{1'b0}};
        for (ii = 0; ii < N; ii = ii + 1) dcnt = dcnt + d[ii];
    end

    wire [DBITS-1:0] d_h = dcnt / 100;
    wire [DBITS-1:0] d_t = (dcnt / 10) % 10;
    wire [DBITS-1:0] d_o = dcnt % 10;

    // ------------------------------------------------------------------
    // UART
    // ------------------------------------------------------------------
    reg        push;
    reg  [7:0] byte_in;
    wire       busy;

    uart_printer u_uart (
        .clk(clk), .rst(rst),
        .push(push), .byte_in(byte_in),
        .busy(busy), .tx(tx)
    );

    // ------------------------------------------------------------------
    // FSM: banner, then print-then-step per generation (UART-paced)
    // ------------------------------------------------------------------
    localparam ST_BANNER = 3'd0;
    localparam ST_LINE   = 3'd1;
    localparam ST_STEP   = 3'd2;
    localparam ST_CSUM   = 3'd3;
    localparam ST_FINAL  = 3'd4;
    localparam ST_DONE   = 3'd5;
    localparam ST_EMIT   = 3'd6;

    reg [2:0]  state, ret_state;
    reg [15:0] gen;
    reg [3:0]  g_t, g_o;           // generation counter as BCD digits
    reg        mode;               // 0 = i%4 field run, 1 = Phi1 clock run
    reg        clock_ok;
    reg [9:0]  line_i, line_len;
    reg [7:0]  line [0:LINE_MAX-1];
    integer    k;

    // Clock-run prediction: even generations all-1, odd all-2
    wire [1:0] exp_clk = g_o[0] ? 2'd2 : 2'd1;

    // N banner digits (3, zero padded) - compile-time constants
    localparam [7:0] N_H = 8'h30 + (((N / 100) % 10));
    localparam [7:0] N_T = 8'h30 + (((N / 10) % 10));
    localparam [7:0] N_O = 8'h30 + ((N % 10));

    always @(posedge clk) begin
        push <= 1'b0;
        if (rst) begin
            state    <= ST_BANNER;
            ret_state<= ST_BANNER;
            for (k = 0; k < N; k = k + 1) x[2*k +: 2] <= (k % 4);
            gen      <= 16'd0;
            g_t      <= 4'd0;
            g_o      <= 4'd0;
            mode     <= 1'b0;
            clock_ok <= 1'b1;
            line_i   <= 10'd0;
            line_len <= 10'd0;
            done     <= 1'b0;
            all_pass <= 1'b0;
        end else begin
            case (state)
                ST_BANNER: begin
                    // "PHASE LATTICE N=XXX\n"
                    line[0]<="P";  line[1]<="H";  line[2]<="A";  line[3]<="S";
                    line[4]<="E";  line[5]<=" "; line[6]<="L";  line[7]<="A";
                    line[8]<="T";  line[9]<="T";  line[10]<="I"; line[11]<="C";
                    line[12]<="E"; line[13]<=" "; line[14]<="N"; line[15]<="=";
                    line[16]<=N_H; line[17]<=N_T; line[18]<=N_O; line[19]<=8'h0A;
                    line_len <= 10'd20; line_i <= 10'd0;
                    ret_state <= ST_LINE; state <= ST_EMIT;
                end

                ST_LINE: begin
                    // "G07 S=<N digits> D=NNN\n" (C-prefix in clock mode)
                    line[0] <= mode ? "C" : "G";
                    line[1] <= dig4(g_t);
                    line[2] <= dig4(g_o);
                    line[3] <= " ";  line[4] <= "S"; line[5] <= "=";
                    for (k = 0; k < N; k = k + 1)
                        line[6+k] <= dig(x[2*k +: 2]);
                    line[6+N]   <= " "; line[6+N+1] <= "D"; line[6+N+2] <= "=";
                    line[6+N+3] <= dig4(d_h[3:0]);
                    line[6+N+4] <= dig4(d_t[3:0]);
                    line[6+N+5] <= dig4(d_o[3:0]);
                    line[6+N+6] <= 8'h0A;
                    line_len <= 6 + N + 7; line_i <= 10'd0;
                    if (mode && (x != {N{exp_clk}})) clock_ok <= 1'b0;
                    ret_state <= ST_STEP; state <= ST_EMIT;
                end

                ST_STEP: begin
                    if (!mode) begin
                        if (gen == GENS - 1) begin
                            mode <= 1'b1;
                            x    <= {N{2'd1}};
                            gen  <= 16'd0; g_t <= 4'd0; g_o <= 4'd0;
                            state <= ST_LINE;
                        end else begin
                            x   <= lw;
                            gen <= gen + 16'd1;
                            if (g_o == 4'd9) begin
                                g_o <= 4'd0; g_t <= g_t + 4'd1;
                            end else g_o <= g_o + 4'd1;
                            state <= ST_LINE;
                        end
                    end else begin
                        if (gen == CLK_GENS - 1) state <= ST_CSUM;
                        else begin
                            x   <= lw;
                            gen <= gen + 16'd1;
                            g_o <= g_o + 4'd1;
                            state <= ST_LINE;
                        end
                    end
                end

                ST_CSUM: begin
                    line[0]<="C"; line[1]<="L"; line[2]<="O"; line[3]<="C";
                    line[4]<="K"; line[5]<=" ";
                    if (clock_ok) begin
                        line[6]<="P"; line[7]<="A"; line[8]<="S"; line[9]<="S";
                    end else begin
                        line[6]<="F"; line[7]<="A"; line[8]<="I"; line[9]<="L";
                    end
                    line[10]<=8'h0A;
                    line_len <= 10'd11; line_i <= 10'd0;
                    ret_state <= ST_FINAL; state <= ST_EMIT;
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
                    all_pass <= clock_ok;
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

// Board top for the scaling study — same shell as phase_lattice_top.v,
// parameterized. NOT flashed (synthesis/PnR resource study only).
module phase_lattice_scale_top #(
    parameter integer N = 16
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

    phase_lattice_uart_scale #(.N(N)) u_lattice (
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
