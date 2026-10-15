// Process phase-state slot transitions (Phi0..Phi3).

module scheduler_unit (
    input  wire       clk,
    input  wire       rst,
    input  wire       req,
    input  wire [1:0] op,
    input  wire [1:0] state_in,
    output reg  [1:0] state_out,
    output reg        accepted
);
    localparam OP_WAKE    = 2'd0;
    localparam OP_SCHED   = 2'd1;
    localparam OP_PREEMPT = 2'd2;
    localparam OP_EXIT    = 2'd3;

  always @(posedge clk) begin
        if (rst) begin
            state_out <= 2'd0;
            accepted  <= 1'b0;
        end else if (req) begin
            accepted <= 1'b0;
            case (op)
                OP_WAKE: begin
                    if (state_in == 2'd0) begin state_out <= 2'd1; accepted <= 1'b1; end
                end
                OP_SCHED: begin
                    if (state_in == 2'd1) begin state_out <= 2'd2; accepted <= 1'b1; end
                end
                OP_PREEMPT: begin
                    if (state_in == 2'd2) begin state_out <= 2'd1; accepted <= 1'b1; end
                end
                OP_EXIT: begin
                    if (state_in != 2'd0) begin state_out <= 2'd0; accepted <= 1'b1; end
                end
                default: accepted <= 1'b0;
            endcase
        end
    end
endmodule
