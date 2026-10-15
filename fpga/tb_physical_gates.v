`timescale 1ns/1ps

module tb_physical_gates;
    reg clk = 0;
    reg rst = 1;
    always #41.667 clk = ~clk; // ~12 MHz

    wire tx, done, all_pass;
    physical_gates_uart dut (
        .clk(clk), .rst(rst), .tx(tx), .done(done), .all_pass(all_pass)
    );

    // Very lightweight UART RX sampler @ 115200
    localparam integer DIV = 104;
    reg [15:0] baud;
    reg [3:0]  bitn;
    reg [7:0]  shifter;
    reg        receiving;
    integer    fd;
    integer    chars;

    initial begin
        fd = $fopen("evidence/reflect_topology/sim_uart.txt", "w");
        chars = 0;
        receiving = 0;
        baud = 0;
        bitn = 0;
        #500;
        rst = 0;
    end

    always @(posedge clk) begin
        if (rst) begin
            receiving <= 0;
        end else if (!receiving) begin
            if (tx === 1'b0) begin
                receiving <= 1;
                baud <= DIV + (DIV/2); // sample mid first data after start
                bitn <= 0;
            end
        end else begin
            if (baud == 0) begin
                if (bitn < 8) begin
                    shifter[bitn] <= tx;
                    bitn <= bitn + 1;
                    baud <= DIV - 1;
                end else begin
                    // stop bit ignored
                    $fwrite(fd, "%c", shifter);
                    chars <= chars + 1;
                    receiving <= 0;
                end
            end else begin
                baud <= baud - 1;
            end
        end

        if (done) begin
            $fwrite(fd, "\n# DONE all_pass=%0d chars=%0d\n", all_pass, chars);
            $fclose(fd);
            if (!all_pass) begin
                $display("FAIL: all_pass=0");
                $finish(1);
            end
            $display("PASS: sim physical gates all_pass=1 chars=%0d", chars);
            $finish(0);
        end
    end

    // Safety timeout (~2 s of simulated time)
    initial begin
        #(2_000_000_000);
        $display("TIMEOUT");
        $finish(1);
    end
endmodule
