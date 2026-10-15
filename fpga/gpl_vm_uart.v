// GPL VM physical gate — run phase_cpu program; report live register result.

module gpl_vm_uart (
    input  wire clk,
    input  wire rst,
    output wire tx,
    output reg  done,
    output reg  all_pass
);
    function automatic [7:0] dig(input [1:0] v);
        dig = 8'h30 + {6'd0, v};
    endfunction

    wire halted;
    wire [1:0] ra, rb, rc, rd;
    wire [31:0] trace_pc;
    wire [4:0]  trace_opcode;
    wire [1:0]  trace_topo;
    wire [15:0] trace_count;

    phase_cpu u_cpu (
        .clk(clk),
        .rst(rst),
        .run(1'b1),
        .halted(halted),
        .reg_a(ra), .reg_b(rb), .reg_c(rc), .reg_d(rd),
        .trace_pc(trace_pc),
        .trace_opcode(trace_opcode),
        .trace_topo(trace_topo),
        .trace_count(trace_count)
    );

    // Matches phase_cpu complex program: A=1 B=0 C=2 D=1
    localparam [1:0] EXP_A = 2'd1;
    localparam [1:0] EXP_B = 2'd0;
    localparam [1:0] EXP_C = 2'd2;
    localparam [1:0] EXP_D = 2'd1;

    reg        push;
    reg  [7:0] byte_in;
    wire       busy;

    uart_printer u_uart (
        .clk(clk), .rst(rst),
        .push(push), .byte_in(byte_in),
        .busy(busy), .tx(tx)
    );

    localparam ST_WAIT   = 3'd0;
    localparam ST_HDR    = 3'd1;
    localparam ST_PROG   = 3'd2;
    localparam ST_REGS   = 3'd3;
    localparam ST_SUM    = 3'd4;
    localparam ST_EMIT   = 3'd5;
    localparam ST_DONE   = 3'd6;

    reg [2:0]  state, ret_state;
    reg [5:0]  idx;
    reg [5:0]  line_i, line_len;
    reg [7:0]  line [0:47];
    reg [7:0]  hdr [0:31];
    reg [4:0]  hdr_len;
    reg [1:0]  la, lb, lc, ld;

    always @(posedge clk) begin
        push <= 1'b0;
        if (rst) begin
            state     <= ST_WAIT;
            ret_state <= ST_WAIT;
            idx       <= 6'd0;
            line_i    <= 5'd0;
            line_len  <= 5'd0;
            done      <= 1'b0;
            all_pass  <= 1'b0;
            la <= 2'd0; lb <= 2'd0; lc <= 2'd0; ld <= 2'd0;
            hdr_len   <= 5'd0;
        end else begin
            case (state)
                ST_WAIT: begin
                    if (halted) begin
                        la <= ra; lb <= rb; lc <= rc; ld <= rd;
                        hdr[0]<="G"; hdr[1]<="P"; hdr[2]<="O"; hdr[3]<="S";
                        hdr[4]<="/"; hdr[5]<="i"; hdr[6]<="C"; hdr[7]<="E";
                        hdr[8]<=" "; hdr[9]<="G"; hdr[10]<="P"; hdr[11]<="L";
                        hdr[12]<=" "; hdr[13]<="V"; hdr[14]<="M"; hdr[15]<=" ";
                        hdr[16]<="T"; hdr[17]<="E"; hdr[18]<="S"; hdr[19]<="T";
                        hdr[20]<=8'h0A;
                        hdr_len <= 5'd21;
                        idx <= 6'd0;
                        state <= ST_HDR;
                    end
                end

                ST_HDR: begin
                    if (!busy && !push) begin
                        if (idx < hdr_len) begin
                            byte_in <= hdr[idx];
                            push    <= 1'b1;
                            idx     <= idx + 6'd1;
                        end else state <= ST_PROG;
                    end
                end

                ST_PROG: begin
                    // "PROG L3 L1 COMP THR ENR REF HALT\n"
                    line[0]<="P"; line[1]<="R"; line[2]<="O"; line[3]<="G";
                    line[4]<=" "; line[5]<="L"; line[6]<="3"; line[7]<=" ";
                    line[8]<="L"; line[9]<="1"; line[10]<=" "; line[11]<="C";
                    line[12]<="O"; line[13]<="M"; line[14]<="P"; line[15]<=" ";
                    line[16]<="T"; line[17]<="H"; line[18]<="R"; line[19]<=" ";
                    line[20]<="E"; line[21]<="N"; line[22]<="R"; line[23]<=" ";
                    line[24]<="R"; line[25]<="E"; line[26]<="F"; line[27]<=" ";
                    line[28]<="H"; line[29]<="A"; line[30]<="L"; line[31]<="T";
                    line[32]<=8'h0A;
                    line_len <= 6'd33; line_i <= 6'd0;
                    ret_state <= ST_REGS; state <= ST_EMIT;
                end

                ST_REGS: begin
                    line[0]<="R"; line[1]<="E"; line[2]<="G"; line[3]<="S";
                    line[4]<=" "; line[5]<="A"; line[6]<="="; line[7]<=dig(la);
                    line[8]<=" "; line[9]<="B"; line[10]<="="; line[11]<=dig(lb);
                    line[12]<=" "; line[13]<="C"; line[14]<="="; line[15]<=dig(lc);
                    line[16]<=" "; line[17]<="D"; line[18]<="="; line[19]<=dig(ld);
                    line[20]<=8'h0A;
                    line_len <= 6'd21; line_i <= 6'd0;
                    ret_state <= ST_SUM; state <= ST_EMIT;
                end

                ST_SUM: begin
                    if ((la == EXP_A) && (lb == EXP_B) && (lc == EXP_C) && (ld == EXP_D)) begin
                        line[0]<="G"; line[1]<="P"; line[2]<="L"; line[3]<=" ";
                        line[4]<="V"; line[5]<="M"; line[6]<=" "; line[7]<="P";
                        line[8]<="A"; line[9]<="S"; line[10]<="S"; line[11]<=8'h0A;
                        line_len <= 6'd12;
                        all_pass <= 1'b1;
                    end else begin
                        line[0]<="G"; line[1]<="P"; line[2]<="L"; line[3]<=" ";
                        line[4]<="V"; line[5]<="M"; line[6]<=" "; line[7]<="F";
                        line[8]<="A"; line[9]<="I"; line[10]<="L"; line[11]<=" ";
                        line[12]<="E"; line[13]<="X"; line[14]<="P"; line[15]<=" ";
                        line[16]<=dig(EXP_A); line[17]<=dig(EXP_B);
                        line[18]<=dig(EXP_C); line[19]<=dig(EXP_D);
                        line[20]<=" "; line[21]<="G"; line[22]<="O"; line[23]<="T";
                        line[24]<=" "; line[25]<=dig(la); line[26]<=dig(lb);
                        line[27]<=dig(lc); line[28]<=dig(ld); line[29]<=8'h0A;
                        line_len <= 6'd30;
                        all_pass <= 1'b0;
                    end
                    line_i <= 6'd0; ret_state <= ST_DONE; state <= ST_EMIT;
                end

                ST_DONE: done <= 1'b1;

                ST_EMIT: begin
                    if (!busy && !push) begin
                        if (line_i < line_len) begin
                            byte_in <= line[line_i];
                            push    <= 1'b1;
                            line_i  <= line_i + 6'd1;
                        end else state <= ret_state;
                    end
                end

                default: state <= ST_DONE;
            endcase
        end
    end

    wire _unused = |{trace_pc, trace_opcode, trace_topo, trace_count};
endmodule
