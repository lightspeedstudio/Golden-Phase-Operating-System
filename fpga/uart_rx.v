// 8N1 UART receiver — 115200 baud at 12 MHz (clk_div = 104).
module uart_rx #(
    parameter CLK_HZ = 12_000_000,
    parameter BAUD   = 115_200
) (
    input  wire       clk,
    input  wire       rst,
    input  wire       rx,
    output reg        valid,
    output reg  [7:0] data
);
    localparam integer DIV = CLK_HZ / BAUD;
    localparam integer HALF = DIV / 2;

    reg [1:0]  rx_sync;
    reg        busy;
    reg [15:0] baud_cnt;
    reg [3:0]  bit_idx;
    reg [7:0]  shifter;

    always @(posedge clk) begin
        valid <= 1'b0;
        if (rst) begin
            rx_sync <= 2'b11;
            busy <= 1'b0;
            baud_cnt <= 16'd0;
            bit_idx <= 4'd0;
            shifter <= 8'd0;
            data <= 8'd0;
        end else begin
            rx_sync <= {rx_sync[0], rx};
            if (!busy) begin
                if (rx_sync == 2'b10) begin // falling edge start
                    busy <= 1'b1;
                    baud_cnt <= HALF; // sample mid of start, then mid of data
                    bit_idx <= 4'd0;
                end
            end else begin
                if (baud_cnt == 16'd0) begin
                    if (bit_idx == 4'd0) begin
                        // mid-start: confirm low, then schedule first data bit
                        if (rx_sync[0] != 1'b0) begin
                            busy <= 1'b0; // framing abort
                        end else begin
                            bit_idx <= 4'd1;
                            baud_cnt <= DIV - 1;
                        end
                    end else if (bit_idx <= 4'd8) begin
                        shifter <= {rx_sync[0], shifter[7:1]};
                        if (bit_idx == 4'd8) begin
                            bit_idx <= 4'd9;
                            baud_cnt <= DIV - 1;
                        end else begin
                            bit_idx <= bit_idx + 4'd1;
                            baud_cnt <= DIV - 1;
                        end
                    end else begin
                        // stop bit
                        data <= shifter;
                        valid <= (rx_sync[0] == 1'b1);
                        busy <= 1'b0;
                    end
                end else baud_cnt <= baud_cnt - 16'd1;
            end
        end
    end
endmodule
