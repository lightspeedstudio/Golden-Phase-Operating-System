// Simple byte RAM for phase CPU.

module phase_memory #(
    parameter ADDR_WIDTH = 12
) (
    input  wire                  clk,
    input  wire                  wen,
    input  wire [ADDR_WIDTH-1:0] addr,
    input  wire [7:0]            wdata,
    output wire [7:0]            rdata
);
    localparam DEPTH = 1 << ADDR_WIDTH;
    reg [7:0] mem [0:DEPTH-1];

    assign rdata = mem[addr];

    always @(posedge clk) begin
        if (wen) mem[addr] <= wdata;
    end
endmodule
