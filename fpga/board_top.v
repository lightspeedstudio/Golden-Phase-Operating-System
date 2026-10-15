// Simulation / generic board top — phase CPU + UART banner.

module board_top (
    input  wire clk,
    input  wire rst_n,
    output wire uart_tx,
    output wire heartbeat
);
    wire rst = ~rst_n;
    wire run = 1'b1;

    wire [31:0] trace_pc;
    wire [4:0]  trace_opcode;
    wire [1:0]  trace_topo;
    wire [15:0] trace_count;
    wire halted;
    wire [1:0] ra, rb, rc, rd;

    phase_cpu #(
        .IMEM_DEPTH(64)
    ) u_cpu (
        .clk(clk),
        .rst(rst),
        .run(run),
        .halted(halted),
        .reg_a(ra), .reg_b(rb), .reg_c(rc), .reg_d(rd),
        .trace_pc(trace_pc),
        .trace_opcode(trace_opcode),
        .trace_topo(trace_topo),
        .trace_count(trace_count)
    );

    reg [23:0] hb_cnt;
    always @(posedge clk) begin
        if (rst) hb_cnt <= 24'd0;
        else hb_cnt <= hb_cnt + 24'd1;
    end
    assign heartbeat = hb_cnt[23];

    reg halted_d;
    always @(posedge clk) begin
        if (rst) halted_d <= 1'b0;
        else halted_d <= halted;
    end
    wire halt_pulse = halted & ~halted_d;

    uart_banner u_uart (
        .clk(clk),
        .rst(rst),
        .halt_pulse(halt_pulse),
        .tx(uart_tx)
    );
endmodule
