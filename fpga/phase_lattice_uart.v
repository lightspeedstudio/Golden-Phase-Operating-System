// Phase lattice CA — 16-cell ring with live causal-curvature (defect) readout.
// Each cell owns 4 combinational compose gates (64 total): both associations
// L=(x[i-1].x[i]).x[i+1] and R=x[i-1].(x[i].x[i+1]) evaluate every cycle in
// parallel; d_i = (L != R) is the defect bit, popcount(d) is the curvature.
// Next state = L (left rule). Lattice math is one cycle; UART is the pacer.

module compose_gate (
    input  wire [1:0] a,
    input  wire [1:0] b,
    output wire [1:0] y
);
    function automatic [1:0] compose_func(input [1:0] aa, input [1:0] bb);
        case ({aa, bb})
            4'b0000: compose_func = 2'd0; 4'b0001: compose_func = 2'd1;
            4'b0010: compose_func = 2'd2; 4'b0011: compose_func = 2'd3;
            4'b0100: compose_func = 2'd1; 4'b0101: compose_func = 2'd3;
            4'b0110: compose_func = 2'd3; 4'b0111: compose_func = 2'd2;
            4'b1000: compose_func = 2'd2; 4'b1001: compose_func = 2'd3;
            4'b1010: compose_func = 2'd3; 4'b1011: compose_func = 2'd1;
            4'b1100: compose_func = 2'd3; 4'b1101: compose_func = 2'd2;
            4'b1110: compose_func = 2'd1; 4'b1111: compose_func = 2'd0;
            default: compose_func = 2'd0;
        endcase
    endfunction
    assign y = compose_func(a, b);
endmodule

module lattice_cell (
    input  wire [1:0] xm,     // x[i-1]
    input  wire [1:0] xc,     // x[i]
    input  wire [1:0] xp,     // x[i+1]
    output wire [1:0] l_out,  // (xm.xc).xp — the left rule / next state
    output wire       defect  // 1 when L != R (local curvature)
);
    wire [1:0] ml, mr, r_out;
    compose_gate u_ml (.a(xm), .b(xc), .y(ml));
    compose_gate u_l  (.a(ml), .b(xp), .y(l_out));
    compose_gate u_mr (.a(xc), .b(xp), .y(mr));
    compose_gate u_r  (.a(xm), .b(mr), .y(r_out));
    assign defect = (l_out != r_out);
endmodule

module phase_lattice_uart (
    input  wire clk,
    input  wire rst,
    output wire tx,
    output reg  done,
    output reg  all_pass
);
    localparam integer N        = 16;
    localparam integer GENS     = 40;
    localparam integer CLK_GENS = 4;

    // Initial field, cell 0 first: 0,1,2,3,3,1,0,2,1,2,3,0,2,0,1,3
    // (flat vector holds cell 15 in the MSBs)
    localparam [31:0] INIT_STATE = {2'd3, 2'd1, 2'd0, 2'd2,
                                    2'd0, 2'd3, 2'd2, 2'd1,
                                    2'd2, 2'd0, 2'd1, 2'd3,
                                    2'd3, 2'd2, 2'd1, 2'd0};
    localparam [31:0] ALL_PHI1   = {16{2'd1}};

    function automatic [7:0] dig(input [1:0] v);
        dig = 8'h30 + {6'd0, v};
    endfunction

    function automatic [7:0] dig4(input [3:0] v);
        dig4 = 8'h30 + {4'd0, v};
    endfunction

    // ------------------------------------------------------------------
    // The lattice: 16 cells x 4 compose gates, all evaluating in parallel
    // ------------------------------------------------------------------
    reg  [31:0] x;
    wire [31:0] lw;
    wire [15:0] d;

    genvar gi;
    generate
        for (gi = 0; gi < N; gi = gi + 1) begin : g_cells
            lattice_cell u_cell (
                .xm(x[2*((gi+N-1)%N)+1 -: 2]),
                .xc(x[2*gi+1 -: 2]),
                .xp(x[2*((gi+1)%N)+1 -: 2]),
                .l_out(lw[2*gi+1 -: 2]),
                .defect(d[gi])
            );
        end
    endgenerate

    // Curvature count = popcount(d), 0..16
    wire [4:0] dcnt = d[0] + d[1] + d[2]  + d[3]  + d[4]  + d[5]  + d[6]  + d[7]
                    + d[8] + d[9] + d[10] + d[11] + d[12] + d[13] + d[14] + d[15];
    wire       d_ten  = (dcnt >= 5'd10);
    wire [4:0] d_rem  = dcnt - 5'd10;
    wire [3:0] d_ones = d_ten ? d_rem[3:0] : dcnt[3:0];

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
    reg [5:0]  gen;
    reg [3:0]  g_t, g_o;          // generation counter as BCD digits
    reg        mode;              // 0 = mixed-field run, 1 = Phi1 clock run
    reg        clock_ok;
    reg [4:0]  line_i, line_len;
    reg [7:0]  line [0:31];
    integer    k;

    // Clock-run prediction: even generations all-1, odd all-2
    wire [1:0] exp_clk = g_o[0] ? 2'd2 : 2'd1;

    always @(posedge clk) begin
        push <= 1'b0;
        if (rst) begin
            state    <= ST_BANNER;
            ret_state<= ST_BANNER;
            x        <= INIT_STATE;
            gen      <= 6'd0;
            g_t      <= 4'd0;
            g_o      <= 4'd0;
            mode     <= 1'b0;
            clock_ok <= 1'b1;
            line_i   <= 5'd0;
            line_len <= 5'd0;
            done     <= 1'b0;
            all_pass <= 1'b0;
        end else begin
            case (state)
                ST_BANNER: begin
                    // "PHASE LATTICE N=16\n"
                    line[0]<="P";  line[1]<="H";  line[2]<="A";  line[3]<="S";
                    line[4]<="E";  line[5]<=" ";  line[6]<="L";  line[7]<="A";
                    line[8]<="T";  line[9]<="T";  line[10]<="I"; line[11]<="C";
                    line[12]<="E"; line[13]<=" "; line[14]<="N"; line[15]<="=";
                    line[16]<="1"; line[17]<="6"; line[18]<=8'h0A;
                    line_len <= 5'd19; line_i <= 5'd0;
                    ret_state <= ST_LINE; state <= ST_EMIT;
                end

                ST_LINE: begin
                    // "G07 S=0123310212302013 D=02\n" (C-prefix in clock mode)
                    line[0] <= mode ? "C" : "G";
                    line[1] <= dig4(g_t);
                    line[2] <= dig4(g_o);
                    line[3] <= " ";  line[4] <= "S"; line[5] <= "=";
                    for (k = 0; k < N; k = k + 1)
                        line[6+k] <= dig(x[2*k+1 -: 2]);
                    line[22] <= " "; line[23] <= "D"; line[24] <= "=";
                    line[25] <= dig4({3'd0, d_ten});
                    line[26] <= dig4(d_ones);
                    line[27] <= 8'h0A;
                    line_len <= 5'd28; line_i <= 5'd0;
                    if (mode && (x != {16{exp_clk}})) clock_ok <= 1'b0;
                    ret_state <= ST_STEP; state <= ST_EMIT;
                end

                ST_STEP: begin
                    if (!mode) begin
                        if (gen == GENS - 1) begin
                            // Switch to experiment 2: homogeneous Phi1 field
                            mode <= 1'b1;
                            x    <= ALL_PHI1;
                            gen  <= 6'd0; g_t <= 4'd0; g_o <= 4'd0;
                            state <= ST_LINE;
                        end else begin
                            x   <= lw;
                            gen <= gen + 6'd1;
                            if (g_o == 4'd9) begin
                                g_o <= 4'd0; g_t <= g_t + 4'd1;
                            end else g_o <= g_o + 4'd1;
                            state <= ST_LINE;
                        end
                    end else begin
                        if (gen == CLK_GENS - 1) state <= ST_CSUM;
                        else begin
                            x   <= lw;
                            gen <= gen + 6'd1;
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
                    line_len <= 5'd11; line_i <= 5'd0;
                    ret_state <= ST_FINAL; state <= ST_EMIT;
                end

                ST_FINAL: begin
                    line[0]<="L";  line[1]<="A";  line[2]<="T";  line[3]<="T";
                    line[4]<="I";  line[5]<="C";  line[6]<="E";  line[7]<=" ";
                    line[8]<="D";  line[9]<="O";  line[10]<="N"; line[11]<="E";
                    line[12]<=8'h0A;
                    line_len <= 5'd13; line_i <= 5'd0;
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
                            line_i  <= line_i + 5'd1;
                        end else state <= ret_state;
                    end
                end

                default: state <= ST_DONE;
            endcase
        end
    end
endmodule
