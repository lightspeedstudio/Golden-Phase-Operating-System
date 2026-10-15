// iCEBreaker top — full physical suite (gates + GPL VM).

module icebreaker_top (
    input  wire clk,
    input  wire btn_n,
    input  wire uart_rx,
    output wire uart_tx,
    output wire ledr_n,
    output wire ledg_n
);
    wire _unused_rx = uart_rx;

    reg [15:0] btn_sync;
    always @(posedge clk) btn_sync <= {btn_sync[14:0], ~btn_n};
    wire btn_rst = (btn_sync == 16'hFFFF);

    reg [13:0] por_cnt = 14'd0;
    reg        por_done = 1'b0;
    always @(posedge clk) begin
        if (!por_done) begin
            if (por_cnt == 14'd12000) por_done <= 1'b1;
            else por_cnt <= por_cnt + 14'd1;
        end
    end

    reg [25:0] resend_cnt;
    reg        force_rst;
    wire       done;
    wire       all_pass;

    wire rst = btn_rst | (~por_done) | force_rst;

    always @(posedge clk) begin
        force_rst <= 1'b0;
        if (btn_rst | (~por_done)) begin
            resend_cnt <= 26'd0;
        end else if (done) begin
            if (resend_cnt == 26'd48_000_000) begin
                resend_cnt <= 26'd0;
                force_rst  <= 1'b1;
            end else begin
                resend_cnt <= resend_cnt + 26'd1;
            end
        end else begin
            resend_cnt <= 26'd0;
        end
    end

    physical_suite_uart u_suite (
        .clk(clk),
        .rst(rst),
        .tx(uart_tx),
        .done(done),
        .all_pass(all_pass)
    );

    reg [23:0] hb_cnt;
    always @(posedge clk) begin
        if (rst) hb_cnt <= 24'd0;
        else hb_cnt <= hb_cnt + 24'd1;
    end
    assign ledg_n = ~(hb_cnt[22]);
    assign ledr_n = ~(done & ~all_pass);

    wire _unused = |{_unused_rx};
endmodule
