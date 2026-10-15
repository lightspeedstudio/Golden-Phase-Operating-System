`timescale 1ns/1ps
// Avenue 9 scaling-study testbench. Override N (and optionally GENS,
// CLK_GENS) from the command line, e.g.:
//   iverilog -g2012 -o tb.vvp -PTB.N=32 uart_tx.v phase_lattice_uart.v \
//       phase_lattice_uart_scale.v tb_phase_lattice_scale.v
// Captures UART bytes to evidence/phase_lattice_scale/sim_uart_N<N>.txt

module tb_phase_lattice_scale;
    parameter N        = 16;
    parameter GENS     = 8;
    parameter CLK_GENS = 4;

    reg clk = 0;
    reg rst = 1;
    always #41.667 clk = ~clk; // ~12 MHz

    wire tx, done, all_pass;
    phase_lattice_uart_scale #(.N(N), .GENS(GENS), .CLK_GENS(CLK_GENS)) dut (
        .clk(clk), .rst(rst), .tx(tx), .done(done), .all_pass(all_pass)
    );

    localparam integer DIV = 104;
    reg [15:0] baud;
    reg [3:0]  bitn;
    reg [7:0]  shifter;
    reg        receiving;
    integer    fd;
    integer    chars;
    reg [1023:0] outpath;

    initial begin
        if (!$value$plusargs("OUT=%s", outpath)) begin
            outpath = "evidence/phase_lattice_scale/sim_uart_default.txt";
        end
        fd = $fopen(outpath, "w");
        if (fd == 0) begin
            $display("FAIL: cannot open output file");
            $finish(1);
        end
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
                baud <= DIV + (DIV/2);
                bitn <= 0;
            end
        end else begin
            if (baud == 0) begin
                if (bitn < 8) begin
                    shifter[bitn] <= tx;
                    bitn <= bitn + 1;
                    baud <= DIV - 1;
                end else begin
                    $fwrite(fd, "%c", shifter);
                    chars <= chars + 1;
                    receiving <= 0;
                end
            end else begin
                baud <= baud - 1;
            end
        end
    end

    initial begin
        wait (done === 1'b1);
        #200000;
        $fclose(fd);
        if (!all_pass) begin
            $display("FAIL: all_pass=0 (CLOCK check failed) N=%0d", N);
            $finish(1);
        end
        $display("PASS: sim phase lattice scale N=%0d all_pass=1 chars=%0d", N, chars);
        $finish(0);
    end

    initial begin
        #(4_000_000_000);
        $display("TIMEOUT N=%0d", N);
        $finish(1);
    end
endmodule
