// Full physical suite: algebra gates -> complex gates -> GPL VM.

module physical_suite_uart (
    input  wire clk,
    input  wire rst,
    output wire tx,
    output reg  done,
    output reg  all_pass
);
    localparam ST_GATES   = 3'd0;
    localparam ST_GAP1    = 3'd1;
    localparam ST_COMPLEX = 3'd2;
    localparam ST_GAP2    = 3'd3;
    localparam ST_VM      = 3'd4;
    localparam ST_DONE    = 3'd5;

    reg [2:0] stage;
    reg [2:0] gap_i;
    reg       g_pass_l;
    reg       c_pass_l;
    reg       v_pass_l;

    wire g_tx, g_done, g_pass;
    wire c_tx, c_done, c_pass;
    wire v_tx, v_done, v_pass;

    wire rst_gates   = rst;
    wire rst_complex = rst | (stage != ST_COMPLEX);
    wire rst_vm      = rst | (stage != ST_VM);

    physical_gates_uart u_gates (
        .clk(clk), .rst(rst_gates),
        .tx(g_tx), .done(g_done), .all_pass(g_pass)
    );

    complex_gates_uart u_complex (
        .clk(clk), .rst(rst_complex),
        .tx(c_tx), .done(c_done), .all_pass(c_pass)
    );

    gpl_vm_uart u_vm (
        .clk(clk), .rst(rst_vm),
        .tx(v_tx), .done(v_done), .all_pass(v_pass)
    );

    reg        push;
    reg  [7:0] byte_in;
    wire       busy;
    wire       gap_tx;

    uart_printer u_gap (
        .clk(clk), .rst(rst),
        .push(push), .byte_in(byte_in),
        .busy(busy), .tx(gap_tx)
    );

    reg [7:0] gap_ch;
    always @(*) begin
        case (gap_i)
            3'd0: gap_ch = "-";
            3'd1: gap_ch = "-";
            3'd2: gap_ch = "-";
            3'd3: gap_ch = "-";
            default: gap_ch = 8'h0A;
        endcase
    end

    assign tx = (stage == ST_GATES)   ? g_tx   :
                (stage == ST_GAP1)    ? gap_tx :
                (stage == ST_COMPLEX) ? c_tx   :
                (stage == ST_GAP2)    ? gap_tx :
                (stage == ST_VM)      ? v_tx   : 1'b1;

    always @(posedge clk) begin
        push <= 1'b0;
        if (rst) begin
            stage    <= ST_GATES;
            gap_i    <= 3'd0;
            done     <= 1'b0;
            all_pass <= 1'b0;
            g_pass_l <= 1'b0;
            c_pass_l <= 1'b0;
            v_pass_l <= 1'b0;
        end else begin
            case (stage)
                ST_GATES: begin
                    if (g_done) begin
                        g_pass_l <= g_pass;
                        gap_i    <= 3'd0;
                        stage    <= ST_GAP1;
                    end
                end
                ST_GAP1: begin
                    if (!busy && !push) begin
                        if (gap_i < 3'd5) begin
                            byte_in <= gap_ch;
                            push    <= 1'b1;
                            gap_i   <= gap_i + 3'd1;
                        end else stage <= ST_COMPLEX;
                    end
                end
                ST_COMPLEX: begin
                    if (c_done) begin
                        c_pass_l <= c_pass;
                        gap_i    <= 3'd0;
                        stage    <= ST_GAP2;
                    end
                end
                ST_GAP2: begin
                    if (!busy && !push) begin
                        if (gap_i < 3'd5) begin
                            byte_in <= gap_ch;
                            push    <= 1'b1;
                            gap_i   <= gap_i + 3'd1;
                        end else stage <= ST_VM;
                    end
                end
                ST_VM: begin
                    if (v_done) begin
                        v_pass_l <= v_pass;
                        stage    <= ST_DONE;
                    end
                end
                ST_DONE: begin
                    done     <= 1'b1;
                    all_pass <= g_pass_l & c_pass_l & v_pass_l;
                end
                default: stage <= ST_GATES;
            endcase
        end
    end
endmodule
