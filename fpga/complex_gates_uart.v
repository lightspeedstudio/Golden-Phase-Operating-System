// Complex physical gates: T_SIGN 4/4, T_ENERGY 4/4, PARALLEL 4x compose.
// Live DUT results from phase_alu instances (not ROM playback).

module complex_gates_uart (
    input  wire clk,
    input  wire rst,
    output wire tx,
    output reg  done,
    output reg  all_pass
);
    function automatic [7:0] dig(input [1:0] v);
        dig = 8'h30 + {6'd0, v};
    endfunction

    function automatic expected_sign(input [1:0] aa);
        expected_sign = (aa == 2'd2) || (aa == 2'd3);
    endfunction

    function automatic expected_energy(input [1:0] aa);
        expected_energy = (aa == 2'd0) || (aa == 2'd3);
    endfunction

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

    // Single-ALU DUT for sign/energy sweeps
    reg  [1:0] a, b;
    wire [1:0] compose_nc, reflect_nc;
    wire       t_sign, t_energy;

    phase_alu u_alu (
        .a(a), .b(b),
        .compose_out(compose_nc),
        .reflect_out(reflect_nc),
        .t_sign_out(t_sign),
        .t_energy_out(t_energy)
    );

    // Parallel 4x compose in one cycle: (0,1)(1,2)(2,3)(3,0)
    wire [1:0] p0, p1, p2, p3;
    wire [1:0] r0, r1, r2, r3;
    wire s0, s1, s2, s3, e0, e1, e2, e3;

    phase_alu u_p0 (.a(2'd0), .b(2'd1), .compose_out(p0), .reflect_out(r0), .t_sign_out(s0), .t_energy_out(e0));
    phase_alu u_p1 (.a(2'd1), .b(2'd2), .compose_out(p1), .reflect_out(r1), .t_sign_out(s1), .t_energy_out(e1));
    phase_alu u_p2 (.a(2'd2), .b(2'd3), .compose_out(p2), .reflect_out(r2), .t_sign_out(s2), .t_energy_out(e2));
    phase_alu u_p3 (.a(2'd3), .b(2'd0), .compose_out(p3), .reflect_out(r3), .t_sign_out(s3), .t_energy_out(e3));

    wire parallel_ok =
        (p0 == expected_compose(2'd0, 2'd1)) &&
        (p1 == expected_compose(2'd1, 2'd2)) &&
        (p2 == expected_compose(2'd2, 2'd3)) &&
        (p3 == expected_compose(2'd3, 2'd0));

    reg        push;
    reg  [7:0] byte_in;
    wire       busy;

    uart_printer u_uart (
        .clk(clk), .rst(rst),
        .push(push), .byte_in(byte_in),
        .busy(busy), .tx(tx)
    );

    localparam ST_HDR_S   = 5'd0;
    localparam ST_S_SET   = 5'd1;
    localparam ST_S_EVAL  = 5'd2;
    localparam ST_S_NEXT  = 5'd3;
    localparam ST_S_SUM   = 5'd4;
    localparam ST_HDR_E   = 5'd5;
    localparam ST_E_SET   = 5'd6;
    localparam ST_E_EVAL  = 5'd7;
    localparam ST_E_NEXT  = 5'd8;
    localparam ST_E_SUM   = 5'd9;
    localparam ST_PAR     = 5'd10;
    localparam ST_PAR_SUM = 5'd11;
    localparam ST_DONE    = 5'd12;
    localparam ST_EMIT    = 5'd13;
    localparam ST_HDR_EMIT = 5'd14;

    reg [4:0]  state, ret_state, hdr_ret;
    reg [5:0]  idx;
    reg [1:0]  case_i;
    reg [4:0]  line_i, line_len, hdr_len;
    reg [7:0]  line [0:31];
    reg [7:0]  hdr [0:31];
    reg        fail_any;

    always @(posedge clk) begin
        push <= 1'b0;
        if (rst) begin
            state     <= ST_HDR_S;
            ret_state <= ST_HDR_S;
            hdr_ret   <= ST_S_SET;
            idx       <= 6'd0;
            case_i    <= 2'd0;
            line_i    <= 5'd0;
            line_len  <= 5'd0;
            hdr_len   <= 5'd0;
            done      <= 1'b0;
            all_pass  <= 1'b0;
            fail_any  <= 1'b0;
            a <= 2'd0; b <= 2'd0;
        end else begin
            case (state)
                ST_HDR_EMIT: begin
                    if (!busy && !push) begin
                        if (idx < hdr_len) begin
                            byte_in <= hdr[idx];
                            push    <= 1'b1;
                            idx     <= idx + 6'd1;
                        end else state <= hdr_ret;
                    end
                end

                ST_HDR_S: begin
                    hdr[0]="G"; hdr[1]="P"; hdr[2]="O"; hdr[3]="S";
                    hdr[4]="/"; hdr[5]="i"; hdr[6]="C"; hdr[7]="E";
                    hdr[8]=" "; hdr[9]="T"; hdr[10]="_"; hdr[11]="S";
                    hdr[12]="I"; hdr[13]="G"; hdr[14]="N"; hdr[15]=" ";
                    hdr[16]="T"; hdr[17]="E"; hdr[18]="S"; hdr[19]="T";
                    hdr[20]=8'h0A;
                    hdr_len <= 5'd21; idx <= 6'd0; case_i <= 2'd0;
                    hdr_ret <= ST_S_SET; state <= ST_HDR_EMIT;
                end

                ST_S_SET: begin
                    a <= case_i; b <= 2'd0;
                    state <= ST_S_EVAL;
                end

                ST_S_EVAL: begin
                    if (t_sign == expected_sign(a)) begin
                        line[0]<=dig(a); line[1]<=" "; line[2]<="S";
                        line[3]<=(t_sign ? 8'h31 : 8'h30);
                        line[4]<=" "; line[5]<="P"; line[6]<="A";
                        line[7]<="S"; line[8]<="S"; line[9]<=8'h0A;
                        line_len<=5'd10;
                    end else begin
                        fail_any <= 1'b1;
                        line[0]<=dig(a); line[1]<=" "; line[2]<="F";
                        line[3]<="A"; line[4]<="I"; line[5]<="L";
                        line[6]<=8'h0A; line_len<=5'd7;
                    end
                    line_i <= 5'd0; ret_state <= ST_S_NEXT; state <= ST_EMIT;
                end

                ST_S_NEXT: begin
                    if (case_i == 2'd3) state <= ST_S_SUM;
                    else begin case_i <= case_i + 2'd1; state <= ST_S_SET; end
                end

                ST_S_SUM: begin
                    line[0]<="T"; line[1]<="_"; line[2]<="S"; line[3]<="I";
                    line[4]<="G"; line[5]<="N"; line[6]<=" "; line[7]<="4";
                    line[8]<="/"; line[9]<="4"; line[10]<=" ";
                    if (fail_any) begin
                        line[11]<="F"; line[12]<="A"; line[13]<="I"; line[14]<="L";
                    end else begin
                        line[11]<="P"; line[12]<="A"; line[13]<="S"; line[14]<="S";
                    end
                    line[15]<=8'h0A; line_len<=5'd16; line_i<=5'd0;
                    ret_state <= ST_HDR_E; state <= ST_EMIT;
                end

                ST_HDR_E: begin
                    hdr[0]="G"; hdr[1]="P"; hdr[2]="O"; hdr[3]="S";
                    hdr[4]="/"; hdr[5]="i"; hdr[6]="C"; hdr[7]="E";
                    hdr[8]=" "; hdr[9]="T"; hdr[10]="_"; hdr[11]="E";
                    hdr[12]="N"; hdr[13]="E"; hdr[14]="R"; hdr[15]="G";
                    hdr[16]="Y"; hdr[17]=" "; hdr[18]="T"; hdr[19]="E";
                    hdr[20]="S"; hdr[21]="T"; hdr[22]=8'h0A;
                    hdr_len <= 5'd23; idx <= 6'd0; case_i <= 2'd0;
                    hdr_ret <= ST_E_SET; state <= ST_HDR_EMIT;
                end

                ST_E_SET: begin
                    a <= case_i; b <= 2'd0;
                    state <= ST_E_EVAL;
                end

                ST_E_EVAL: begin
                    if (t_energy == expected_energy(a)) begin
                        line[0]<=dig(a); line[1]<=" "; line[2]<="E";
                        line[3]<=(t_energy ? 8'h31 : 8'h30);
                        line[4]<=" "; line[5]<="P"; line[6]<="A";
                        line[7]<="S"; line[8]<="S"; line[9]<=8'h0A;
                        line_len<=5'd10;
                    end else begin
                        fail_any <= 1'b1;
                        line[0]<=dig(a); line[1]<=" "; line[2]<="F";
                        line[3]<="A"; line[4]<="I"; line[5]<="L";
                        line[6]<=8'h0A; line_len<=5'd7;
                    end
                    line_i <= 5'd0; ret_state <= ST_E_NEXT; state <= ST_EMIT;
                end

                ST_E_NEXT: begin
                    if (case_i == 2'd3) state <= ST_E_SUM;
                    else begin case_i <= case_i + 2'd1; state <= ST_E_SET; end
                end

                ST_E_SUM: begin
                    line[0]<="T"; line[1]<="_"; line[2]<="E"; line[3]<="N";
                    line[4]<="E"; line[5]<="R"; line[6]<="G"; line[7]<="Y";
                    line[8]<=" "; line[9]<="4"; line[10]<="/"; line[11]<="4";
                    line[12]<=" ";
                    if (fail_any) begin
                        line[13]<="F"; line[14]<="A"; line[15]<="I"; line[16]<="L";
                    end else begin
                        line[13]<="P"; line[14]<="A"; line[15]<="S"; line[16]<="S";
                    end
                    line[17]<=8'h0A; line_len<=5'd18; line_i<=5'd0;
                    ret_state <= ST_PAR; state <= ST_EMIT;
                end

                ST_PAR: begin
                    // One-cycle parallel sample line: "PAR p0 p1 p2 p3\n"
                    line[0]<="P"; line[1]<="A"; line[2]<="R"; line[3]<=" ";
                    line[4]<=dig(p0); line[5]<=" "; line[6]<=dig(p1);
                    line[7]<=" "; line[8]<=dig(p2); line[9]<=" ";
                    line[10]<=dig(p3); line[11]<=8'h0A;
                    line_len<=5'd12; line_i<=5'd0;
                    if (!parallel_ok) fail_any <= 1'b1;
                    ret_state <= ST_PAR_SUM; state <= ST_EMIT;
                end

                ST_PAR_SUM: begin
                    line[0]<="P"; line[1]<="A"; line[2]<="R"; line[3]<="A";
                    line[4]<="L"; line[5]<="L"; line[6]<="E"; line[7]<="L";
                    line[8]<=" "; line[9]<="4"; line[10]<="/"; line[11]<="4";
                    line[12]<=" ";
                    if (!parallel_ok || fail_any) begin
                        line[13]<="F"; line[14]<="A"; line[15]<="I"; line[16]<="L";
                    end else begin
                        line[13]<="P"; line[14]<="A"; line[15]<="S"; line[16]<="S";
                    end
                    line[17]<=8'h0A; line_len<=5'd18; line_i<=5'd0;
                    ret_state <= ST_DONE; state <= ST_EMIT;
                end

                ST_DONE: begin
                    done     <= 1'b1;
                    all_pass <= ~fail_any & parallel_ok;
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

    wire _unused = |{compose_nc, reflect_nc, r0, r1, r2, r3, s0, s1, s2, s3, e0, e1, e2, e3};
endmodule
