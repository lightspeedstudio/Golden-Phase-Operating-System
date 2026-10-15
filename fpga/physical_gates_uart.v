// Physical gates suite: COMPOSE 16/16 -> REFLECT 4/4 -> TOPOLOGY 64/64.
// Results printed from live FPGA logic (phase_alu / topology_unit), not ROM playback.

module physical_gates_uart (
    input  wire clk,
    input  wire rst,
    output wire tx,
    output reg  done,
    output reg  all_pass
);
    function automatic [1:0] expected_compose(input [1:0] aa, input [1:0] bb);
        case ({aa, bb})
            4'b0000: expected_compose = 2'd0; 4'b0001: expected_compose = 2'd1;
            4'b0010: expected_compose = 2'd2; 4'b0011: expected_compose = 2'd3;
            4'b0100: expected_compose = 2'd1; 4'b0101: expected_compose = 2'd3;
            4'b0110: expected_compose = 2'd3; 4'b0111: expected_compose = 2'd2;
            4'b1000: expected_compose = 2'd2; 4'b1001: expected_compose = 2'd3;
            4'b1010: expected_compose = 2'd3; 4'b1011: expected_compose = 2'd1;
            4'b1100: expected_compose = 2'd3; 4'b1101: expected_compose = 2'd2;
            4'b1110: expected_compose = 2'd1; 4'b1111: expected_compose = 2'd0;
            default: expected_compose = 2'd0;
        endcase
    endfunction

    function automatic [1:0] expected_reflect(input [1:0] aa);
        expected_reflect = 2'd3 - aa;
    endfunction

    // Expected topology status from frozen tables (compare only).
    function automatic [1:0] expected_topo_status(input [1:0] aa, input [1:0] bb, input [1:0] cc);
        if (expected_compose(expected_compose(aa, bb), cc)
                == expected_compose(aa, expected_compose(bb, cc)))
            expected_topo_status = 2'd0;
        else if (expected_compose(expected_compose(aa, bb), cc)
                == expected_reflect(expected_compose(aa, expected_compose(bb, cc))))
            expected_topo_status = 2'd1;
        else
            expected_topo_status = 2'd2;
    endfunction

    function automatic [7:0] dig(input [1:0] v);
        dig = 8'h30 + {6'd0, v};
    endfunction

    // ---- DUT wiring ----
    reg  [1:0] a, b, c;
    wire [1:0] compose_out, reflect_out;
    wire       t_sign_nc, t_energy_nc;
    wire [1:0] topo_L, topo_R, topo_status;

    phase_alu u_alu (
        .a(a), .b(b),
        .compose_out(compose_out),
        .reflect_out(reflect_out),
        .t_sign_out(t_sign_nc),
        .t_energy_out(t_energy_nc)
    );

    topology_unit u_topo (
        .a(a), .b(b), .c(c),
        .check_L(topo_L),
        .check_R(topo_R),
        .status(topo_status)
    );

    reg        push;
    reg  [7:0] byte_in;
    wire       busy;

    uart_printer u_uart (
        .clk(clk), .rst(rst),
        .push(push), .byte_in(byte_in),
        .busy(busy), .tx(tx)
    );

    // Header fragments as inline emit states
    localparam ST_HDR_C   = 5'd0;
    localparam ST_C_SET   = 5'd1;
    localparam ST_C_EVAL  = 5'd2;
    localparam ST_C_NEXT  = 5'd3;
    localparam ST_C_SUM   = 5'd4;
    localparam ST_HDR_R   = 5'd5;
    localparam ST_R_SET   = 5'd6;
    localparam ST_R_EVAL  = 5'd7;
    localparam ST_R_NEXT  = 5'd8;
    localparam ST_R_SUM   = 5'd9;
    localparam ST_HDR_T   = 5'd10;
    localparam ST_T_SET   = 5'd11;
    localparam ST_T_EVAL  = 5'd12;
    localparam ST_T_NEXT  = 5'd13;
    localparam ST_T_SUM   = 5'd14;
    localparam ST_HALT    = 5'd15;
    localparam ST_DONE    = 5'd16;
    localparam ST_EMIT    = 5'd17;
    localparam ST_HDR_EMIT = 5'd18;

    reg [4:0]  state, ret_state;
    reg [5:0]  idx;
    reg [3:0]  case_i;   // compose 0..15, reflect 0..3
    reg [5:0]  topo_i;   // 0..63
    reg [4:0]  line_i, line_len;
    reg [7:0]  line [0:39];
    reg        fail_any;
    reg [7:0]  hdr [0:31];
    reg [4:0]  hdr_len;
    reg [4:0]  hdr_ret;

    always @(posedge clk) begin
        push <= 1'b0;
        if (rst) begin
            state     <= ST_HDR_C;
            ret_state <= ST_HDR_C;
            idx       <= 6'd0;
            case_i    <= 4'd0;
            topo_i    <= 6'd0;
            line_i    <= 5'd0;
            line_len  <= 5'd0;
            a <= 2'd0; b <= 2'd0; c <= 2'd0;
            fail_any  <= 1'b0;
            done      <= 1'b0;
            all_pass  <= 1'b0;
            hdr_len   <= 5'd0;
            hdr_ret   <= ST_C_SET;
        end else begin
            case (state)
                // ---- COMPOSE header ----
                ST_HDR_C: begin
                    // "GPOS/iCE COMPOSE TEST\n"
                    hdr[0]="G"; hdr[1]="P"; hdr[2]="O"; hdr[3]="S";
                    hdr[4]="/"; hdr[5]="i"; hdr[6]="C"; hdr[7]="E";
                    hdr[8]=" "; hdr[9]="C"; hdr[10]="O"; hdr[11]="M";
                    hdr[12]="P"; hdr[13]="O"; hdr[14]="S"; hdr[15]="E";
                    hdr[16]=" "; hdr[17]="T"; hdr[18]="E"; hdr[19]="S";
                    hdr[20]="T"; hdr[21]=8'h0A;
                    hdr_len <= 5'd22;
                    idx     <= 6'd0;
                    hdr_ret <= ST_C_SET;
                    case_i  <= 4'd0;
                    state   <= ST_HDR_EMIT;
                end

                ST_HDR_EMIT: begin
                    if (!busy && !push) begin
                        if (idx < hdr_len) begin
                            byte_in <= hdr[idx];
                            push    <= 1'b1;
                            idx     <= idx + 6'd1;
                        end else begin
                            state <= hdr_ret;
                        end
                    end
                end

                ST_C_SET: begin
                    a <= case_i[3:2];
                    b <= case_i[1:0];
                    state <= ST_C_EVAL;
                end

                ST_C_EVAL: begin
                    if (compose_out == expected_compose(a, b)) begin
                        line[0]<=dig(a); line[1]<=dig(b); line[2]<=" ";
                        line[3]<="-"; line[4]<=">"; line[5]<=" ";
                        line[6]<=dig(compose_out); line[7]<=" ";
                        line[8]<="P"; line[9]<="A"; line[10]<="S"; line[11]<="S";
                        line[12]<=8'h0A; line_len<=5'd13;
                    end else begin
                        fail_any <= 1'b1;
                        line[0]<=dig(a); line[1]<=dig(b); line[2]<=" ";
                        line[3]<="E"; line[4]<="X"; line[5]<="P"; line[6]<="E";
                        line[7]<="C"; line[8]<="T"; line[9]<="E"; line[10]<="D";
                        line[11]<=" "; line[12]<=dig(expected_compose(a,b));
                        line[13]<=" "; line[14]<="A"; line[15]<="C"; line[16]<="T";
                        line[17]<="U"; line[18]<="A"; line[19]<="L"; line[20]<=" ";
                        line[21]<=dig(compose_out); line[22]<=" ";
                        line[23]<="F"; line[24]<="A"; line[25]<="I"; line[26]<="L";
                        line[27]<=8'h0A; line_len<=5'd28;
                    end
                    line_i <= 5'd0; ret_state <= ST_C_NEXT; state <= ST_EMIT;
                end

                ST_C_NEXT: begin
                    if (case_i == 4'd15) state <= ST_C_SUM;
                    else begin case_i <= case_i + 4'd1; state <= ST_C_SET; end
                end

                ST_C_SUM: begin
                    line[0]<="C"; line[1]<="O"; line[2]<="M"; line[3]<="P";
                    line[4]<="O"; line[5]<="S"; line[6]<="E"; line[7]<=" ";
                    line[8]<="1"; line[9]<="6"; line[10]<="/"; line[11]<="1";
                    line[12]<="6"; line[13]<=" ";
                    if (fail_any) begin
                        line[14]<="F"; line[15]<="A"; line[16]<="I"; line[17]<="L";
                    end else begin
                        line[14]<="P"; line[15]<="A"; line[16]<="S"; line[17]<="S";
                    end
                    line[18]<=8'h0A; line_len<=5'd19; line_i<=5'd0;
                    ret_state <= ST_HDR_R; state <= ST_EMIT;
                end

                // ---- REFLECT header ----
                ST_HDR_R: begin
                    hdr[0]="G"; hdr[1]="P"; hdr[2]="O"; hdr[3]="S";
                    hdr[4]="/"; hdr[5]="i"; hdr[6]="C"; hdr[7]="E";
                    hdr[8]=" "; hdr[9]="R"; hdr[10]="E"; hdr[11]="F";
                    hdr[12]="L"; hdr[13]="E"; hdr[14]="C"; hdr[15]="T";
                    hdr[16]=" "; hdr[17]="T"; hdr[18]="E"; hdr[19]="S";
                    hdr[20]="T"; hdr[21]=8'h0A;
                    hdr_len <= 5'd22; idx <= 6'd0; case_i <= 4'd0;
                    hdr_ret <= ST_R_SET; state <= ST_HDR_EMIT;
                end

                ST_R_SET: begin
                    a <= case_i[1:0];
                    b <= 2'd0;
                    state <= ST_R_EVAL;
                end

                ST_R_EVAL: begin
                    if (reflect_out == expected_reflect(a)) begin
                        line[0]<=dig(a); line[1]<=" "; line[2]<="-"; line[3]<=">";
                        line[4]<=" "; line[5]<=dig(reflect_out); line[6]<=" ";
                        line[7]<="P"; line[8]<="A"; line[9]<="S"; line[10]<="S";
                        line[11]<=8'h0A; line_len<=5'd12;
                    end else begin
                        fail_any <= 1'b1;
                        line[0]<=dig(a); line[1]<=" ";
                        line[2]<="E"; line[3]<="X"; line[4]<="P"; line[5]<="E";
                        line[6]<="C"; line[7]<="T"; line[8]<="E"; line[9]<="D";
                        line[10]<=" "; line[11]<=dig(expected_reflect(a));
                        line[12]<=" "; line[13]<="A"; line[14]<="C"; line[15]<="T";
                        line[16]<="U"; line[17]<="A"; line[18]<="L"; line[19]<=" ";
                        line[20]<=dig(reflect_out); line[21]<=" ";
                        line[22]<="F"; line[23]<="A"; line[24]<="I"; line[25]<="L";
                        line[26]<=8'h0A; line_len<=5'd27;
                    end
                    line_i <= 5'd0; ret_state <= ST_R_NEXT; state <= ST_EMIT;
                end

                ST_R_NEXT: begin
                    if (case_i == 4'd3) state <= ST_R_SUM;
                    else begin case_i <= case_i + 4'd1; state <= ST_R_SET; end
                end

                ST_R_SUM: begin
                    line[0]<="R"; line[1]<="E"; line[2]<="F"; line[3]<="L";
                    line[4]<="E"; line[5]<="C"; line[6]<="T"; line[7]<=" ";
                    line[8]<="4"; line[9]<="/"; line[10]<="4"; line[11]<=" ";
                    if (fail_any) begin
                        line[12]<="F"; line[13]<="A"; line[14]<="I"; line[15]<="L";
                    end else begin
                        line[12]<="P"; line[13]<="A"; line[14]<="S"; line[15]<="S";
                    end
                    line[16]<=8'h0A; line_len<=5'd17; line_i<=5'd0;
                    ret_state <= ST_HDR_T; state <= ST_EMIT;
                end

                // ---- TOPOLOGY header ----
                ST_HDR_T: begin
                    hdr[0]="G"; hdr[1]="P"; hdr[2]="O"; hdr[3]="S";
                    hdr[4]="/"; hdr[5]="i"; hdr[6]="C"; hdr[7]="E";
                    hdr[8]=" "; hdr[9]="T"; hdr[10]="O"; hdr[11]="P";
                    hdr[12]="O"; hdr[13]="L"; hdr[14]="O"; hdr[15]="G";
                    hdr[16]="Y"; hdr[17]=" "; hdr[18]="T"; hdr[19]="E";
                    hdr[20]="S"; hdr[21]="T"; hdr[22]=8'h0A;
                    hdr_len <= 5'd23; idx <= 6'd0; topo_i <= 6'd0;
                    hdr_ret <= ST_T_SET; state <= ST_HDR_EMIT;
                end

                ST_T_SET: begin
                    a <= topo_i[5:4];
                    b <= topo_i[3:2];
                    c <= topo_i[1:0];
                    state <= ST_T_EVAL;
                end

                ST_T_EVAL: begin
                    if (topo_status == expected_topo_status(a, b, c)) begin
                        // "ABC S0 PASS\n" or "ABC S1 PASS\n"  (status from HW)
                        line[0]<=dig(a); line[1]<=dig(b); line[2]<=dig(c);
                        line[3]<=" "; line[4]<="S"; line[5]<=dig(topo_status);
                        line[6]<=" "; line[7]<="P"; line[8]<="A"; line[9]<="S";
                        line[10]<="S"; line[11]<=8'h0A; line_len<=5'd12;
                    end else begin
                        fail_any <= 1'b1;
                        // "ABC EXPECTED Es ACTUAL As FAIL\n"
                        line[0]<=dig(a); line[1]<=dig(b); line[2]<=dig(c);
                        line[3]<=" "; line[4]<="E"; line[5]<="X"; line[6]<="P";
                        line[7]<="E"; line[8]<="C"; line[9]<="T"; line[10]<="E";
                        line[11]<="D"; line[12]<=" ";
                        line[13]<=dig(expected_topo_status(a,b,c));
                        line[14]<=" "; line[15]<="A"; line[16]<="C"; line[17]<="T";
                        line[18]<="U"; line[19]<="A"; line[20]<="L"; line[21]<=" ";
                        line[22]<=dig(topo_status); line[23]<=" ";
                        line[24]<="F"; line[25]<="A"; line[26]<="I"; line[27]<="L";
                        line[28]<=8'h0A; line_len<=5'd29;
                    end
                    line_i <= 5'd0; ret_state <= ST_T_NEXT; state <= ST_EMIT;
                end

                ST_T_NEXT: begin
                    if (topo_i == 6'd63) state <= ST_T_SUM;
                    else begin topo_i <= topo_i + 6'd1; state <= ST_T_SET; end
                end

                ST_T_SUM: begin
                    line[0]<="T"; line[1]<="O"; line[2]<="P"; line[3]<="O";
                    line[4]<="L"; line[5]<="O"; line[6]<="G"; line[7]<="Y";
                    line[8]<=" "; line[9]<="6"; line[10]<="4"; line[11]<="/";
                    line[12]<="6"; line[13]<="4"; line[14]<=" ";
                    if (fail_any) begin
                        line[15]<="F"; line[16]<="A"; line[17]<="I"; line[18]<="L";
                    end else begin
                        line[15]<="P"; line[16]<="A"; line[17]<="S"; line[18]<="S";
                    end
                    line[19]<=8'h0A; line_len<=5'd20; line_i<=5'd0;
                    ret_state <= ST_HALT; state <= ST_EMIT;
                end

                ST_HALT: begin
                    line[0]<="G"; line[1]<="P"; line[2]<="O"; line[3]<="S";
                    line[4]<="/"; line[5]<="i"; line[6]<="C"; line[7]<="E";
                    line[8]<=" "; line[9]<="H"; line[10]<="A"; line[11]<="L";
                    line[12]<="T"; line[13]<="E"; line[14]<="D"; line[15]<=8'h0A;
                    line_len<=5'd16; line_i<=5'd0;
                    ret_state <= ST_DONE; state <= ST_EMIT;
                end

                ST_DONE: begin
                    done     <= 1'b1;
                    all_pass <= ~fail_any;
                end

                ST_EMIT: begin
                    if (!busy && !push) begin
                        if (line_i < line_len) begin
                            byte_in <= line[line_i];
                            push    <= 1'b1;
                            line_i  <= line_i + 5'd1;
                        end else begin
                            state <= ret_state;
                        end
                    end
                end

                default: state <= ST_DONE;
            endcase
        end
    end
endmodule
