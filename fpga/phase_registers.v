// A/B/C/D phase register file.

module phase_registers (
    input  wire        clk,
    input  wire        rst,
    input  wire        wen,
    input  wire [1:0]  wreg,
    input  wire [1:0]  wdata,
    output wire [1:0]  ra,
    output wire [1:0]  rb,
    output wire [1:0]  rc,
    output wire [1:0]  rd
);
    reg [1:0] regs [0:3];
    integer i;

    always @(posedge clk) begin
        if (rst) begin
            for (i = 0; i < 4; i = i + 1) regs[i] <= 2'd0;
        end else if (wen) begin
            regs[wreg] <= wdata;
        end
    end

    assign ra = regs[0];
    assign rb = regs[1];
    assign rc = regs[2];
    assign rd = regs[3];
endmodule
