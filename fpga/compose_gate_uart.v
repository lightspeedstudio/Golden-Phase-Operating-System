// Physical compose gate: all 16 pairs through phase_alu; UART evidence.
// Expected table is compare-only. Printed digit is always `actual` from ALU.
module compose_gate_uart (
    input  wire clk,
    input  wire rst,
    output wire tx,
    output reg  done,
    output reg  all_pass
);
    function automatic [1:0] expected_compose(input [1:0] aa, input [1:0] bb);
        case ({aa, bb})
            4'b0000: expected_compose = 2'd0;
            4'b0001: expected_compose = 2'd1;
            4'b0010: expected_compose = 2'd2;
            4'b0011: expected_compose = 2'd3;
            4'b0100: expected_compose = 2'd1;
            4'b0101: expected_compose = 2'd3;
            4'b0110: expected_compose = 2'd3;
            4'b0111: expected_compose = 2'd2;
            4'b1000: expected_compose = 2'd2;
            4'b1001: expected_compose = 2'd3;
            4'b1010: expected_compose = 2'd3;
            4'b1011: expected_compose = 2'd1;
            4'b1100: expected_compose = 2'd3;
            4'b1101: expected_compose = 2'd2;
            4'b1110: expected_compose = 2'd1;
            4'b1111: expected_compose = 2'd0;
            default: expected_compose = 2'd0;
        endcase
    endfunction

    function automatic [7:0] dig(input [1:0] v);
        dig = 8'h30 + {6'd0, v};
    endfunction

    reg  [1:0] a;
    reg  [1:0] b;
    wire [1:0] actual;
    wire [1:0] reflect_nc;
    wire       t_sign_nc, t_energy_nc;

    phase_alu u_alu (
        .a(a),
        .b(b),
        .compose_out(actual),
        .reflect_out(reflect_nc),
        .t_sign_out(t_sign_nc),
        .t_energy_out(t_energy_nc)
    );

    wire [1:0] expect  = expected_compose(a, b);
    wire       pair_ok = (actual == expect);

    reg        push;
    reg  [7:0] byte_in;
    wire       busy;

    uart_printer u_uart (
        .clk(clk),
        .rst(rst),
        .push(push),
        .byte_in(byte_in),
        .busy(busy),
        .tx(tx)
    );

    localparam integer HDR_LEN = 22; // includes trailing \n
    reg [7:0] hdr [0:HDR_LEN-1];
    initial begin
        hdr[0]="G"; hdr[1]="P"; hdr[2]="O"; hdr[3]="S";
        hdr[4]="/"; hdr[5]="i"; hdr[6]="C"; hdr[7]="E";
        hdr[8]=" "; hdr[9]="C"; hdr[10]="O"; hdr[11]="M";
        hdr[12]="P"; hdr[13]="O"; hdr[14]="S"; hdr[15]="E";
        hdr[16]=" "; hdr[17]="T"; hdr[18]="E"; hdr[19]="S";
        hdr[20]="T"; hdr[21]=8'h0A;
    end

    localparam S_HDR   = 3'd0;
    localparam S_SET   = 3'd1;
    localparam S_EVAL  = 3'd2;
    localparam S_EMIT  = 3'd3;
    localparam S_NEXT  = 3'd4;
    localparam S_SUM   = 3'd5;
    localparam S_HALT  = 3'd6;
    localparam S_DONE  = 3'd7;

    reg [2:0]  state;
    reg [2:0]  ret_state;
    reg [5:0]  idx;
    reg [3:0]  case_i;
    reg [4:0]  line_i;
    reg [4:0]  line_len;
    reg [7:0]  line [0:31];
    reg        fail_any;
    reg        latch_ok;
    reg [1:0]  latch_act;
    reg [1:0]  latch_exp;
    reg [1:0]  latch_a;
    reg [1:0]  latch_b;

    always @(posedge clk) begin
        push <= 1'b0;
        if (rst) begin
            state     <= S_HDR;
            ret_state <= S_HDR;
            idx       <= 6'd0;
            case_i    <= 4'd0;
            line_i    <= 5'd0;
            line_len  <= 5'd0;
            a         <= 2'd0;
            b         <= 2'd0;
            fail_any  <= 1'b0;
            done      <= 1'b0;
            all_pass  <= 1'b0;
            latch_ok  <= 1'b0;
            latch_act <= 2'd0;
            latch_exp <= 2'd0;
            latch_a   <= 2'd0;
            latch_b   <= 2'd0;
        end else begin
            case (state)
                S_HDR: begin
                    if (!busy && !push) begin
                        if (idx < HDR_LEN) begin
                            byte_in <= hdr[idx];
                            push    <= 1'b1;
                            idx     <= idx + 6'd1;
                        end else begin
                            idx    <= 6'd0;
                            case_i <= 4'd0;
                            state  <= S_SET;
                        end
                    end
                end

                S_SET: begin
                    a <= case_i[3:2];
                    b <= case_i[1:0];
                    state <= S_EVAL;
                end

                S_EVAL: begin
                    // Latch ALU result (combinational from phase_alu) after a/b stable.
                    latch_a   <= a;
                    latch_b   <= b;
                    latch_act <= actual;
                    latch_exp <= expect;
                    latch_ok  <= pair_ok;
                    if (!pair_ok) fail_any <= 1'b1;

                    if (pair_ok) begin
                        // "AB -> C PASS\n"
                        line[0]  <= dig(a);
                        line[1]  <= dig(b);
                        line[2]  <= " ";
                        line[3]  <= "-";
                        line[4]  <= ">";
                        line[5]  <= " ";
                        line[6]  <= dig(actual);
                        line[7]  <= " ";
                        line[8]  <= "P";
                        line[9]  <= "A";
                        line[10] <= "S";
                        line[11] <= "S";
                        line[12] <= 8'h0A;
                        line_len <= 5'd13;
                    end else begin
                        // "AB EXPECTED E ACTUAL A FAIL\n"
                        line[0]  <= dig(a);
                        line[1]  <= dig(b);
                        line[2]  <= " ";
                        line[3]  <= "E";
                        line[4]  <= "X";
                        line[5]  <= "P";
                        line[6]  <= "E";
                        line[7]  <= "C";
                        line[8]  <= "T";
                        line[9]  <= "E";
                        line[10] <= "D";
                        line[11] <= " ";
                        line[12] <= dig(expect);
                        line[13] <= " ";
                        line[14] <= "A";
                        line[15] <= "C";
                        line[16] <= "T";
                        line[17] <= "U";
                        line[18] <= "A";
                        line[19] <= "L";
                        line[20] <= " ";
                        line[21] <= dig(actual);
                        line[22] <= " ";
                        line[23] <= "F";
                        line[24] <= "A";
                        line[25] <= "I";
                        line[26] <= "L";
                        line[27] <= 8'h0A;
                        line_len <= 5'd28;
                    end
                    line_i    <= 5'd0;
                    ret_state <= S_NEXT;
                    state     <= S_EMIT;
                end

                S_NEXT: begin
                    if (case_i == 4'd15) begin
                        state <= S_SUM;
                    end else begin
                        case_i <= case_i + 4'd1;
                        state  <= S_SET;
                    end
                end

                S_SUM: begin
                    line[0]  <= "C";
                    line[1]  <= "O";
                    line[2]  <= "M";
                    line[3]  <= "P";
                    line[4]  <= "O";
                    line[5]  <= "S";
                    line[6]  <= "E";
                    line[7]  <= " ";
                    line[8]  <= "1";
                    line[9]  <= "6";
                    line[10] <= "/";
                    line[11] <= "1";
                    line[12] <= "6";
                    line[13] <= " ";
                    if (fail_any) begin
                        line[14] <= "F";
                        line[15] <= "A";
                        line[16] <= "I";
                        line[17] <= "L";
                    end else begin
                        line[14] <= "P";
                        line[15] <= "A";
                        line[16] <= "S";
                        line[17] <= "S";
                    end
                    line[18]  <= 8'h0A;
                    line_len  <= 5'd19;
                    line_i    <= 5'd0;
                    ret_state <= S_HALT;
                    state     <= S_EMIT;
                end

                S_HALT: begin
                    line[0]  <= "G";
                    line[1]  <= "P";
                    line[2]  <= "O";
                    line[3]  <= "S";
                    line[4]  <= "/";
                    line[5]  <= "i";
                    line[6]  <= "C";
                    line[7]  <= "E";
                    line[8]  <= " ";
                    line[9]  <= "H";
                    line[10] <= "A";
                    line[11] <= "L";
                    line[12] <= "T";
                    line[13] <= "E";
                    line[14] <= "D";
                    line[15] <= 8'h0A;
                    line_len <= 5'd16;
                    line_i   <= 5'd0;
                    ret_state <= S_DONE;
                    state     <= S_EMIT;
                end

                S_DONE: begin
                    done     <= 1'b1;
                    all_pass <= ~fail_any;
                end

                S_EMIT: begin
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

                default: state <= S_DONE;
            endcase
        end
    end
endmodule
