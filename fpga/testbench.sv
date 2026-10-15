`timescale 1ns/1ps

module testbench;
    reg clk = 0;
    reg rst = 1;
    always #5 clk = ~clk;

    wire uart_tx;
    wire heartbeat;

    board_top dut (
        .clk(clk),
        .rst_n(~rst),
        .uart_tx(uart_tx),
        .heartbeat(heartbeat)
    );

    initial begin
        $dumpfile("kairos_fpga.vcd");
        $dumpvars(0, testbench);
        #20 rst = 0;
        #2000;
        if (dut.u_cpu.halted !== 1'b1) begin
            $display("FAIL: CPU did not halt");
            $finish(1);
        end
        if (dut.u_cpu.reg_a !== 2'd1 || dut.u_cpu.reg_b !== 2'd2 ||
            dut.u_cpu.reg_c !== 2'd3 || dut.u_cpu.reg_d !== 2'd0) begin
            $display("FAIL: regs A=%0d B=%0d C=%0d D=%0d",
                dut.u_cpu.reg_a, dut.u_cpu.reg_b, dut.u_cpu.reg_c, dut.u_cpu.reg_d);
            $finish(1);
        end
        $display("PASS: FPGA simulation GPL VM regs OK");
        $finish(0);
    end
endmodule
