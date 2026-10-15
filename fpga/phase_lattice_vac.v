// Homogeneous vacuum/lock lattice test: all cells = PHI, expect fixed field + D=00.
// Reuses compose_gate / lattice_cell from phase_lattice_uart.v (read together at synth).

module phase_lattice_vac_uart #(
    parameter integer PHI  = 0,   // 0 = vacuum Phi0, 3 = lock Phi3
    parameter integer N    = 16,
    parameter integer GENS = 16
) (
    input  wire clk,
    input  wire rst,
    output wire tx,
    output reg  done,
    output reg  all_pass
);
    localparam [31:0] INIT = {16{PHI[1:0]}};

    function automatic [7:0] dig(input [1:0] v);
        dig = 8'h30 + {6'd0, v};
    endfunction
    function automatic [7:0] dig4(input [3:0] v);
        dig4 = 8'h30 + {4'd0, v};
    endfunction

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

    wire [4:0] dcnt = d[0] + d[1] + d[2]  + d[3]  + d[4]  + d[5]  + d[6]  + d[7]
                    + d[8] + d[9] + d[10] + d[11] + d[12] + d[13] + d[14] + d[15];
    wire       d_ten  = (dcnt >= 5'd10);
    wire [4:0] d_rem  = dcnt - 5'd10;
    wire [3:0] d_ones = d_ten ? d_rem[3:0] : dcnt[3:0];

    reg        push;
    reg  [7:0] byte_in;
    wire       busy;
    uart_printer u_uart (
        .clk(clk), .rst(rst),
        .push(push), .byte_in(byte_in),
        .busy(busy), .tx(tx)
    );

    localparam ST_BANNER = 3'd0;
    localparam ST_LINE   = 3'd1;
    localparam ST_STEP   = 3'd2;
    localparam ST_CSUM   = 3'd3;
    localparam ST_FINAL  = 3'd4;
    localparam ST_DONE   = 3'd5;
    localparam ST_EMIT   = 3'd6;

    reg [2:0]  state, ret_state;
    reg [5:0]  gen;
    reg [3:0]  g_t, g_o;
    reg        vac_ok;
    reg [4:0]  line_i, line_len;
    reg [7:0]  line [0:31];
    integer    k;

    always @(posedge clk) begin
        push <= 1'b0;
        if (rst) begin
            state    <= ST_BANNER;
            ret_state<= ST_BANNER;
            x        <= INIT;
            gen      <= 6'd0;
            g_t      <= 4'd0;
            g_o      <= 4'd0;
            vac_ok   <= 1'b1;
            line_i   <= 5'd0;
            line_len <= 5'd0;
            done     <= 1'b0;
            all_pass <= 1'b0;
        end else begin
            case (state)
                ST_BANNER: begin
                    // "PHASE VACUUM PhiX\n"
                    line[0]<="P"; line[1]<="H"; line[2]<="A"; line[3]<="S";
                    line[4]<="E"; line[5]<=" "; line[6]<="V"; line[7]<="A";
                    line[8]<="C"; line[9]<="U"; line[10]<="U"; line[11]<="M";
                    line[12]<=" "; line[13]<="P"; line[14]<="h"; line[15]<="i";
                    line[16]<=dig(PHI[1:0]); line[17]<=8'h0A;
                    line_len <= 5'd18; line_i <= 5'd0;
                    ret_state <= ST_LINE; state <= ST_EMIT;
                end

                ST_LINE: begin
                    line[0]<="G"; line[1]<=dig4(g_t); line[2]<=dig4(g_o);
                    line[3]<=" "; line[4]<="S"; line[5]<="=";
                    for (k = 0; k < N; k = k + 1)
                        line[6+k] <= dig(x[2*k+1 -: 2]);
                    line[22]<=" "; line[23]<="D"; line[24]<="=";
                    line[25]<=dig4({3'd0, d_ten});
                    line[26]<=dig4(d_ones);
                    line[27]<=8'h0A;
                    line_len <= 5'd28; line_i <= 5'd0;
                    if ((x != INIT) || (dcnt != 5'd0)) vac_ok <= 1'b0;
                    ret_state <= ST_STEP; state <= ST_EMIT;
                end

                ST_STEP: begin
                    if (gen == GENS - 1) state <= ST_CSUM;
                    else begin
                        x   <= lw;
                        gen <= gen + 6'd1;
                        if (g_o == 4'd9) begin
                            g_o <= 4'd0; g_t <= g_t + 4'd1;
                        end else g_o <= g_o + 4'd1;
                        state <= ST_LINE;
                    end
                end

                ST_CSUM: begin
                    line[0]<="V"; line[1]<="A"; line[2]<="C"; line[3]<="U";
                    line[4]<="U"; line[5]<="M"; line[6]<=" ";
                    if (vac_ok) begin
                        line[7]<="P"; line[8]<="A"; line[9]<="S"; line[10]<="S";
                    end else begin
                        line[7]<="F"; line[8]<="A"; line[9]<="I"; line[10]<="L";
                    end
                    line[11]<=8'h0A;
                    line_len <= 5'd12; line_i <= 5'd0;
                    ret_state <= ST_FINAL; state <= ST_EMIT;
                end

                ST_FINAL: begin
                    line[0]<="L"; line[1]<="A"; line[2]<="T"; line[3]<="T";
                    line[4]<="I"; line[5]<="C"; line[6]<="E"; line[7]<=" ";
                    line[8]<="D"; line[9]<="O"; line[10]<="N"; line[11]<="E";
                    line[12]<=8'h0A;
                    line_len <= 5'd13; line_i <= 5'd0;
                    ret_state <= ST_DONE; state <= ST_EMIT;
                end

                ST_DONE: begin
                    done     <= 1'b1;
                    all_pass <= vac_ok;
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

module phase_lattice_vac_top #(
    parameter integer PHI = 0
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
        if (btn_rst | (~por_done)) resend_cnt <= 26'd0;
        else if (done) begin
            if (resend_cnt == 26'd48_000_000) begin
                resend_cnt <= 26'd0;
                force_rst  <= 1'b1;
            end else resend_cnt <= resend_cnt + 26'd1;
        end else resend_cnt <= 26'd0;
    end

    phase_lattice_vac_uart #(.PHI(PHI)) u_lattice (
        .clk(clk), .rst(rst), .tx(uart_tx), .done(done), .all_pass(all_pass)
    );

    reg [23:0] hb_cnt;
    always @(posedge clk) begin
        if (rst) hb_cnt <= 24'd0;
        else hb_cnt <= hb_cnt + 24'd1;
    end
    assign ledg_n = ~(hb_cnt[22]);
    assign ledr_n = ~(done & ~all_pass);
endmodule
