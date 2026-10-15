// Page tag transitions for four-state memory model.

module memory_unit (
    input  wire       clk,
    input  wire       rst,
    input  wire       req,
    input  wire [2:0] op,
    input  wire [1:0] tag_in,
    output reg  [1:0] tag_out,
    output reg        accepted
);
    localparam OP_MAP_USER   = 3'd0;
    localparam OP_MAP_KERNEL = 3'd1;
    localparam OP_LOCK       = 3'd2;
    localparam OP_UNLOCK     = 3'd3;
    localparam OP_FREE       = 3'd4;

    always @(posedge clk) begin
        if (rst) begin
            tag_out  <= 2'd0;
            accepted <= 1'b0;
        end else if (req) begin
            accepted <= 1'b0;
            case (op)
                OP_MAP_USER: begin
                    if (tag_in == 2'd0) begin tag_out <= 2'd1; accepted <= 1'b1; end
                end
                OP_MAP_KERNEL: begin
                    if (tag_in == 2'd0) begin tag_out <= 2'd2; accepted <= 1'b1; end
                end
                OP_LOCK: begin
                    if (tag_in == 2'd2) begin tag_out <= 2'd3; accepted <= 1'b1; end
                end
                OP_UNLOCK: begin
                    if (tag_in == 2'd3) begin tag_out <= 2'd2; accepted <= 1'b1; end
                end
                OP_FREE: begin
                    if (tag_in == 2'd1) begin tag_out <= 2'd0; accepted <= 1'b1; end
                end
                default: accepted <= 1'b0;
            endcase
        end
    end
endmodule
