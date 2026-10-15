// 8N1 UART transmitter — 115200 baud at 12 MHz (clk_div = 104).

module uart_tx #(
    parameter CLK_HZ  = 12_000_000,
    parameter BAUD    = 115_200
) (
    input  wire       clk,
    input  wire       rst,
    input  wire       start,
    input  wire [7:0] data,
    output reg        busy,
    output reg        tx
);
    localparam integer DIV = CLK_HZ / BAUD; // 104 @ 12 MHz / 115200

    reg [15:0] baud_cnt;
    reg [3:0]  bit_idx;
    reg [9:0]  shifter;

    always @(posedge clk) begin
        if (rst) begin
            tx       <= 1'b1;
            busy     <= 1'b0;
            baud_cnt <= 16'd0;
            bit_idx  <= 4'd0;
            shifter  <= 10'h3FF;
        end else if (!busy) begin
            if (start) begin
                busy     <= 1'b1;
                shifter  <= {1'b1, data, 1'b0}; // stop, data[7:0], start
                bit_idx  <= 4'd0;
                baud_cnt <= DIV - 1;
                tx       <= 1'b0; // drive start bit immediately
            end
        end else begin
            if (baud_cnt == 16'd0) begin
                if (bit_idx == 4'd9) begin
                    busy <= 1'b0;
                    tx   <= 1'b1;
                end else begin
                    shifter  <= {1'b1, shifter[9:1]};
                    tx       <= shifter[1]; // next bit after shift-in-progress
                    bit_idx  <= bit_idx + 4'd1;
                    baud_cnt <= DIV - 1;
                end
            end else begin
                baud_cnt <= baud_cnt - 16'd1;
            end
        end
    end
endmodule

module uart_printer (
    input  wire       clk,
    input  wire       rst,
    input  wire       push,
    input  wire [7:0] byte_in,
    output wire       busy,
    output wire       tx
);
    uart_tx u_tx (
        .clk(clk),
        .rst(rst),
        .start(push),
        .data(byte_in),
        .busy(busy),
        .tx(tx)
    );
endmodule

// Sends a fixed banner once after reset, then again when halt_pulse fires.
module uart_banner (
    input  wire clk,
    input  wire rst,
    input  wire halt_pulse,
    output wire tx
);
    // "GPOS/iCE HALTED\n"
    localparam integer MSG_LEN = 16;
    reg [7:0] msg [0:MSG_LEN-1];
    initial begin
        msg[0]  = "G"; msg[1]  = "P"; msg[2]  = "O"; msg[3]  = "S";
        msg[4]  = "/"; msg[5]  = "i"; msg[6]  = "C"; msg[7]  = "E";
        msg[8]  = " "; msg[9]  = "H"; msg[10] = "A"; msg[11] = "L";
        msg[12] = "T"; msg[13] = "E"; msg[14] = "D";
        msg[15] = 8'h0A;
    end

    reg        start;
    reg [7:0]  data;
    wire       busy;
    reg [4:0]  idx;
    reg        send_boot;
    reg        send_halt;
    reg [1:0]  state;

    localparam S_IDLE = 2'd0;
    localparam S_LOAD = 2'd1;
    localparam S_WAIT = 2'd2;

    uart_tx u_tx (
        .clk(clk), .rst(rst),
        .start(start), .data(data),
        .busy(busy), .tx(tx)
    );

    always @(posedge clk) begin
        start <= 1'b0;
        if (rst) begin
            idx       <= 5'd0;
            state     <= S_IDLE;
            send_boot <= 1'b1;
            send_halt <= 1'b0;
        end else begin
            if (halt_pulse) send_halt <= 1'b1;

            case (state)
                S_IDLE: begin
                    if (send_boot || send_halt) begin
                        idx   <= 5'd0;
                        state <= S_LOAD;
                        if (send_boot) send_boot <= 1'b0;
                        else send_halt <= 1'b0;
                    end
                end
                S_LOAD: begin
                    if (!busy) begin
                        data  <= msg[idx];
                        start <= 1'b1;
                        state <= S_WAIT;
                    end
                end
                S_WAIT: begin
                    if (!busy && !start) begin
                        if (idx == MSG_LEN - 1) begin
                            state <= S_IDLE;
                        end else begin
                            idx   <= idx + 5'd1;
                            state <= S_LOAD;
                        end
                    end
                end
                default: state <= S_IDLE;
            endcase
        end
    end
endmodule
