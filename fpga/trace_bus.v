// External debug trace bus.

module trace_bus (
    input  wire        clk,
    input  wire        rst,
    input  wire        push,
    input  wire [31:0] pc,
    input  wire [4:0]  opcode,
    input  wire [1:0]  phase_a,
    input  wire [1:0]  phase_b,
    input  wire [1:0]  topo_status,
    output reg  [31:0] trace_pc,
    output reg  [4:0]  trace_opcode,
    output reg  [1:0]  trace_topo,
    output reg  [15:0] trace_count
);
    always @(posedge clk) begin
        if (rst) begin
            trace_pc     <= 32'd0;
            trace_opcode <= 5'd0;
            trace_topo   <= 2'd0;
            trace_count  <= 16'd0;
        end else if (push) begin
            trace_pc     <= pc;
            trace_opcode <= opcode;
            trace_topo   <= topo_status;
            trace_count  <= trace_count + 16'd1;
        end
    end
endmodule
