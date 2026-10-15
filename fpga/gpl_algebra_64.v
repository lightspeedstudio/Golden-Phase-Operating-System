// Phase 2 — full GPL algebra golden on hardware (64 triples).
// For each (a,b,c): print ab, L=(a∘b)∘c, R=a∘(b∘c), p, PASS/FAIL.
// Compose computed live via compose_gate (same primitive as lattice).

module gpl_algebra_64_uart (
    input  wire clk,
    input  wire rst,
    output wire tx,
    output reg  done,
    output reg  all_pass
);
    function automatic [7:0] dig(input [1:0] v);
        dig = 8'h30 + {6'd0, v};
    endfunction

    reg  [1:0] a, b, c;
    wire [1:0] ab, bc, L, R;
    compose_gate u_ab (.a(a), .b(b), .y(ab));
    compose_gate u_bc (.a(b), .b(c), .y(bc));
    compose_gate u_L  (.a(ab), .b(c), .y(L));
    compose_gate u_R  (.a(a), .b(bc), .y(R));
    wire [1:0] R_ref = 2'd3 - R;
    wire       p0 = (L == R);
    wire       p1 = (L == R_ref);
    wire       ok = p0 | p1;
    wire [1:0] p  = p0 ? 2'd0 : (p1 ? 2'd1 : 2'd2);

    reg        push;
    reg  [7:0] byte_in;
    wire       busy;
    uart_printer u_uart (.clk(clk), .rst(rst), .push(push), .byte_in(byte_in), .busy(busy), .tx(tx));

    localparam ST_HDR   = 3'd0;
    localparam ST_SET   = 3'd1;
    localparam ST_EVAL  = 3'd2;
    localparam ST_NEXT  = 3'd3;
    localparam ST_SUM   = 3'd4;
    localparam ST_DONE  = 3'd5;
    localparam ST_EMIT  = 3'd6;

    reg [2:0]  state, ret_state;
    reg [5:0]  idx;
    reg [5:0]  pass_n;
    reg [5:0]  line_i, line_len;
    reg [7:0]  line [0:47];
    reg        fail_any;

    always @(posedge clk) begin
        push <= 1'b0;
        if (rst) begin
            state <= ST_HDR; ret_state <= ST_HDR;
            idx <= 6'd0; pass_n <= 6'd0;
            a <= 2'd0; b <= 2'd0; c <= 2'd0;
            fail_any <= 1'b0; done <= 1'b0; all_pass <= 1'b0;
            line_i <= 6'd0; line_len <= 6'd0;
        end else begin
            case (state)
                ST_HDR: begin
                    line[0]<="G"; line[1]<="P"; line[2]<="L"; line[3]<=" ";
                    line[4]<="A"; line[5]<="L"; line[6]<="G"; line[7]<="E";
                    line[8]<="B"; line[9]<="R"; line[10]<="A"; line[11]<=" ";
                    line[12]<="6"; line[13]<="4"; line[14]<=" ";
                    line[15]<="T"; line[16]<="R"; line[17]<="I"; line[18]<="P";
                    line[19]<="L"; line[20]<="E"; line[21]<="S"; line[22]<=8'h0A;
                    line_len <= 6'd23; line_i <= 6'd0;
                    idx <= 6'd0; pass_n <= 6'd0; fail_any <= 1'b0;
                    ret_state <= ST_SET; state <= ST_EMIT;
                end
                ST_SET: begin
                    a <= idx[5:4];
                    b <= idx[3:2];
                    c <= idx[1:0];
                    state <= ST_EVAL;
                end
                ST_EVAL: begin
                    // "T abc ab=X L=Y R=Z p=P PASS\n"
                    line[0]<="T"; line[1]<=" ";
                    line[2]<=dig(a); line[3]<=dig(b); line[4]<=dig(c); line[5]<=" ";
                    line[6]<="a"; line[7]<="b"; line[8]<="="; line[9]<=dig(ab); line[10]<=" ";
                    line[11]<="L"; line[12]<="="; line[13]<=dig(L); line[14]<=" ";
                    line[15]<="R"; line[16]<="="; line[17]<=dig(R); line[18]<=" ";
                    line[19]<="p"; line[20]<="="; line[21]<=dig(p[1:0]); line[22]<=" ";
                    if (ok) begin
                        line[23]<="P"; line[24]<="A"; line[25]<="S"; line[26]<="S";
                        pass_n <= pass_n + 6'd1;
                    end else begin
                        line[23]<="F"; line[24]<="A"; line[25]<="I"; line[26]<="L";
                        fail_any <= 1'b1;
                    end
                    line[27]<=8'h0A;
                    line_len <= 6'd28; line_i <= 6'd0;
                    ret_state <= ST_NEXT; state <= ST_EMIT;
                end
                ST_NEXT: begin
                    if (idx == 6'd63) state <= ST_SUM;
                    else begin idx <= idx + 6'd1; state <= ST_SET; end
                end
                ST_SUM: begin
                    line[0]<="A"; line[1]<="L"; line[2]<="G"; line[3]<="E";
                    line[4]<="B"; line[5]<="R"; line[6]<="A"; line[7]<=" ";
                    line[8]<="6"; line[9]<="4"; line[10]<="/"; line[11]<="6";
                    line[12]<="4"; line[13]<=" ";
                    if (!fail_any) begin
                        line[14]<="P"; line[15]<="A"; line[16]<="S"; line[17]<="S";
                    end else begin
                        line[14]<="F"; line[15]<="A"; line[16]<="I"; line[17]<="L";
                    end
                    line[18]<=8'h0A;
                    line[19]<="A"; line[20]<="L"; line[21]<="G"; line[22]<="E";
                    line[23]<="B"; line[24]<="R"; line[25]<="A"; line[26]<=" ";
                    line[27]<="D"; line[28]<="O"; line[29]<="N"; line[30]<="E";
                    line[31]<=8'h0A;
                    line_len <= 6'd32; line_i <= 6'd0;
                    ret_state <= ST_DONE; state <= ST_EMIT;
                end
                ST_DONE: begin
                    done <= 1'b1;
                    all_pass <= ~fail_any;
                end
                ST_EMIT: begin
                    if (!busy && !push) begin
                        if (line_i < line_len) begin
                            byte_in <= line[line_i];
                            push <= 1'b1;
                            line_i <= line_i + 6'd1;
                        end else state <= ret_state;
                    end
                end
                default: state <= ST_DONE;
            endcase
        end
    end
endmodule

module gpl_algebra_64_top (
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

    gpl_algebra_64_uart u_alg (
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
